package ipc

import (
	"net"
	"path/filepath"
	"testing"
)

func serveOnce(t *testing.T, sock string, fn func(Request) Response) {
	t.Helper()
	l, err := net.Listen("unix", sock)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { l.Close() })
	go func() {
		conn, err := l.Accept()
		if err != nil {
			return
		}
		defer conn.Close()
		req, err := ReadRequest(conn)
		if err != nil {
			_ = WriteResponse(conn, ErrorResponse("%v", err))
			return
		}
		_ = WriteResponse(conn, fn(req))
	}()
}

func TestSendRoundTrip(t *testing.T) {
	sock := filepath.Join(t.TempDir(), "z.sock")
	serveOnce(t, sock, func(req Request) Response {
		if req.Op != OpStatus || req.Target != "network" {
			return ErrorResponse("unexpected %v", req)
		}
		return OKResponse(map[string]string{"kind": "wifi"})
	})
	resp, err := Send(sock, Request{Op: OpStatus, Target: "network"})
	if err != nil {
		t.Fatal(err)
	}
	if !resp.OK {
		t.Fatalf("not ok: %s", resp.Error)
	}
	var data map[string]string
	if err := resp.DecodeData(&data); err != nil {
		t.Fatal(err)
	}
	if data["kind"] != "wifi" {
		t.Fatalf("got %v", data)
	}
}

func TestSendDialError(t *testing.T) {
	sock := filepath.Join(t.TempDir(), "missing.sock")
	if _, err := Send(sock, Request{Op: OpPing}); err == nil {
		t.Fatal("expected dial error")
	}
}

func TestErrorResponseDecode(t *testing.T) {
	resp := ErrorResponse("boom %d", 42)
	var v map[string]string
	if err := resp.DecodeData(&v); err == nil || err.Error() != "boom 42" {
		t.Fatalf("got %v", err)
	}
}

func TestPingDead(t *testing.T) {
	if Ping(filepath.Join(t.TempDir(), "missing.sock")) {
		t.Fatal("expected ping to fail")
	}
}
