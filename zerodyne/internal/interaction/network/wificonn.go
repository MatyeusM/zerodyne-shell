package network

import (
	"context"
	"fmt"
	"strings"

	"zerodyne/internal/interaction"
)

// ConnectHandler implements "wifi-connect <ssid> [password]".
type ConnectHandler struct {
	run runFunc
}

// NewConnectHandler builds the handler with live system access.
func NewConnectHandler() *ConnectHandler { return &ConnectHandler{run: liveRun} }

// NewConnectHandlerWithRunner builds the handler with a stub runner (tests).
func NewConnectHandlerWithRunner(run runFunc) *ConnectHandler { return &ConnectHandler{run: run} }

// Name implements interaction.Handler.
func (h *ConnectHandler) Name() string { return "wifi-connect" }

// Usage implements interaction.Handler.
func (h *ConnectHandler) Usage() string { return "wifi-connect <ssid> [password]" }

// NeedPassword is the error text when the AP demands credentials QML must
// prompt for. QML matches this exact string.
const NeedPassword = "need-password"

// Exec implements interaction.Handler.
func (h *ConnectHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) < 1 || len(args) > 2 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	cmd := []string{"dev", "wifi", "connect", args[0]}
	if len(args) == 2 {
		cmd = append(cmd, "password", args[1])
	}
	out, err := h.run(ctx, "nmcli", cmd...)
	if err != nil {
		if strings.Contains(out, "Secrets were required") {
			return interaction.Result{}, fmt.Errorf("%s", NeedPassword)
		}
		return interaction.Result{}, fmt.Errorf("wifi-connect: %s", firstLine(out))
	}
	return interaction.Result{Message: "connected to " + args[0]}, nil
}

// DisconnectHandler implements "wifi-disconnect".
type DisconnectHandler struct {
	run runFunc
}

// NewDisconnectHandler builds the handler with live system access.
func NewDisconnectHandler() *DisconnectHandler { return &DisconnectHandler{run: liveRun} }

// NewDisconnectHandlerWithRunner builds the handler with a stub runner (tests).
func NewDisconnectHandlerWithRunner(run runFunc) *DisconnectHandler {
	return &DisconnectHandler{run: run}
}

// Name implements interaction.Handler.
func (h *DisconnectHandler) Name() string { return "wifi-disconnect" }

// Usage implements interaction.Handler.
func (h *DisconnectHandler) Usage() string { return "wifi-disconnect" }

// Exec implements interaction.Handler.
func (h *DisconnectHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 0 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	out, err := nmList(ctx, h.run, "DEVICE,TYPE,STATE", "device", "status")
	if err != nil {
		return interaction.Result{}, fmt.Errorf("wifi-disconnect: %w", err)
	}
	dev := ""
	for _, line := range strings.Split(out, "\n") {
		parts := strings.SplitN(line, ":", 3)
		if len(parts) == 3 && parts[1] == "wifi" && strings.HasPrefix(parts[2], "connected") {
			dev = parts[0]
			break
		}
	}
	if dev == "" {
		return interaction.Result{}, fmt.Errorf("wifi-disconnect: no active wifi connection")
	}
	if out, err := h.run(ctx, "nmcli", "dev", "disconnect", dev); err != nil {
		return interaction.Result{}, fmt.Errorf("wifi-disconnect: %s", firstLine(out))
	}
	return interaction.Result{Message: "disconnected"}, nil
}

// ForgetHandler implements "wifi-forget <ssid>": deletes the saved profile
// whose wireless SSID (or name) matches.
type ForgetHandler struct {
	run runFunc
}

// NewForgetHandler builds the handler with live system access.
func NewForgetHandler() *ForgetHandler { return &ForgetHandler{run: liveRun} }

// NewForgetHandlerWithRunner builds the handler with a stub runner (tests).
func NewForgetHandlerWithRunner(run runFunc) *ForgetHandler { return &ForgetHandler{run: run} }

// Name implements interaction.Handler.
func (h *ForgetHandler) Name() string { return "wifi-forget" }

// Usage implements interaction.Handler.
func (h *ForgetHandler) Usage() string { return "wifi-forget <ssid>" }

// Exec implements interaction.Handler.
func (h *ForgetHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 1 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	out, err := nmList(ctx, h.run, "NAME,802-11-wireless.ssid", "connection", "show")
	if err != nil {
		return interaction.Result{}, fmt.Errorf("wifi-forget: %w", err)
	}
	profile := ""
	for _, line := range strings.Split(out, "\n") {
		name, ssid, _ := strings.Cut(line, ":")
		name, ssid = strings.TrimSpace(name), strings.TrimSpace(ssid)
		if name == "" {
			continue
		}
		if ssid == args[0] || name == args[0] {
			profile = name
			break
		}
	}
	if profile == "" {
		return interaction.Result{}, fmt.Errorf("wifi-forget: no saved network %q", args[0])
	}
	if out, err := h.run(ctx, "nmcli", "connection", "delete", profile); err != nil {
		return interaction.Result{}, fmt.Errorf("wifi-forget: %s", firstLine(out))
	}
	return interaction.Result{Message: "forgot " + args[0]}, nil
}

func firstLine(s string) string {
	if i := strings.Index(s, "\n"); i >= 0 {
		return strings.TrimSpace(s[:i])
	}
	return strings.TrimSpace(s)
}
