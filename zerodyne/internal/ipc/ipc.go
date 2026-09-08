// Package ipc defines the local JSON protocol spoken over the daemon's
// Unix socket. One connection carries exactly one request and one response,
// each a single JSON object terminated by '\n'.
//
// Requests:
//
//	{"op":"status","target":"network"}            snapshot a monitor
//	{"op":"status"}                               daemon info
//	{"op":"exec","target":"wifi-band","args":[]}  run an interaction handler
//	{"op":"ping"}                                 liveness probe
//	{"op":"stop"}                                 ask the daemon to exit
//
// The protocol is transport code only: dispatch lives in the daemon package
// so CLI, Quickshell, and any other local client share the same surface.
package ipc

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"time"
)

// Ops understood by the daemon.
const (
	OpStatus = "status"
	OpExec   = "exec"
	OpPing   = "ping"
	OpStop   = "stop"
)

// Timeouts for a single request/response exchange.
const (
	DialTimeout = 2 * time.Second
	IOTimeout   = 5 * time.Second
	MaxLine     = 256 * 1024
)

// Request is a single client request.
type Request struct {
	Op     string   `json:"op"`
	Target string   `json:"target,omitempty"`
	Args   []string `json:"args,omitempty"`
}

// Response is a single daemon reply. Exactly one of Data / Error is set.
type Response struct {
	OK    bool            `json:"ok"`
	Data  json.RawMessage `json:"data,omitempty"`
	Error string          `json:"error,omitempty"`
}

// ErrorResponse builds a failure response.
func ErrorResponse(format string, args ...any) Response {
	return Response{OK: false, Error: fmt.Sprintf(format, args...)}
}

// OKResponse marshals v as the payload of a success response.
func OKResponse(v any) Response {
	raw, err := json.Marshal(v)
	if err != nil {
		return ErrorResponse("encode response: %v", err)
	}
	return Response{OK: true, Data: raw}
}

// DecodeData unmarshals a success response payload into v.
func (r Response) DecodeData(v any) error {
	if !r.OK {
		if r.Error == "" {
			return fmt.Errorf("daemon error")
		}
		return fmt.Errorf("%s", r.Error)
	}
	if len(r.Data) == 0 {
		return nil
	}
	return json.Unmarshal(r.Data, v)
}

// Send dials sockPath, writes req, and reads the single-line response.
func Send(sockPath string, req Request) (Response, error) {
	conn, err := net.DialTimeout("unix", sockPath, DialTimeout)
	if err != nil {
		return Response{}, err
	}
	defer conn.Close()

	if err := conn.SetDeadline(time.Now().Add(IOTimeout)); err != nil {
		return Response{}, err
	}
	raw, err := json.Marshal(req)
	if err != nil {
		return Response{}, err
	}
	raw = append(raw, '\n')
	if _, err := conn.Write(raw); err != nil {
		return Response{}, err
	}
	line, err := bufio.NewReader(conn).ReadBytes('\n')
	if err != nil {
		return Response{}, err
	}
	var resp Response
	if err := json.Unmarshal(line, &resp); err != nil {
		return Response{}, fmt.Errorf("decode response: %w", err)
	}
	return resp, nil
}

// ReadRequest reads one newline-terminated request from conn.
func ReadRequest(conn net.Conn) (Request, error) {
	var req Request
	if err := conn.SetDeadline(time.Now().Add(IOTimeout)); err != nil {
		return req, err
	}
	reader := bufio.NewReader(io.LimitReader(conn, MaxLine+1))
	line, err := reader.ReadBytes('\n')
	if err != nil {
		return req, fmt.Errorf("read request: %w", err)
	}
	if err := json.Unmarshal(line, &req); err != nil {
		return req, fmt.Errorf("decode request: %w", err)
	}
	return req, nil
}

// WriteResponse writes one newline-terminated response to conn.
func WriteResponse(conn net.Conn, resp Response) error {
	if err := conn.SetDeadline(time.Now().Add(IOTimeout)); err != nil {
		return err
	}
	raw, err := json.Marshal(resp)
	if err != nil {
		return err
	}
	raw = append(raw, '\n')
	_, err = conn.Write(raw)
	return err
}

// Ping reports whether a daemon answers at sockPath.
func Ping(sockPath string) bool {
	resp, err := Send(sockPath, Request{Op: OpPing})
	return err == nil && resp.OK
}
