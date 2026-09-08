package bluetooth

import (
	"context"
	"fmt"
	"regexp"
	"time"

	"zerodyne/internal/interaction"
)

// DeviceHandler implements
// "bluetooth-device [pair|connect|disconnect|forget] <address>", ported
// from omarchy-bluetooth-device.
//
// Steps are best-effort like the script (`|| true` everywhere): the UI
// polls adapter/device state afterwards, so slow BlueZ sequencing reports
// through state, not the exit code. Only usage errors fail.
type DeviceHandler struct {
	run runFunc
}

// NewDeviceHandler builds the handler with live system access.
func NewDeviceHandler() *DeviceHandler { return &DeviceHandler{run: liveRun} }

// NewDeviceHandlerWithRunner builds the handler with a stub runner (tests).
func NewDeviceHandlerWithRunner(run runFunc) *DeviceHandler { return &DeviceHandler{run: run} }

// Name implements interaction.Handler.
func (h *DeviceHandler) Name() string { return "bluetooth-device" }

// Usage implements interaction.Handler.
func (h *DeviceHandler) Usage() string {
	return "bluetooth-device [pair|connect|disconnect|forget] <address>"
}

var addrRe = regexp.MustCompile(`^([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}$`)

// Exec implements interaction.Handler.
func (h *DeviceHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 2 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	action, address := args[0], args[1]
	switch action {
	case "pair", "connect", "disconnect", "forget":
	default:
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	if !addrRe.MatchString(address) {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	switch action {
	case "pair":
		ensurePowered(ctx, h.run)
		pair(ctx, h.run, address)
		trust(ctx, h.run, address)
		connect(ctx, h.run, address)
	case "connect":
		ensurePowered(ctx, h.run)
		trust(ctx, h.run, address)
		connect(ctx, h.run, address)
	case "disconnect":
		disconnect(ctx, h.run, address)
	case "forget":
		ensurePowered(ctx, h.run)
		disconnect(ctx, h.run, address)
		remove(ctx, h.run, address)
	}
	return interaction.Result{Message: action + " " + address}, nil
}

func pair(ctx context.Context, run runFunc, address string) {
	sctx, cancel := step(ctx, 20*time.Second)
	defer cancel()
	_, _ = run(sctx, "bluetoothctl", "pair", address)
}

func trust(ctx context.Context, run runFunc, address string) {
	sctx, cancel := step(ctx, 5*time.Second)
	defer cancel()
	_, _ = run(sctx, "bluetoothctl", "trust", address)
}

func connect(ctx context.Context, run runFunc, address string) {
	sctx, cancel := step(ctx, 20*time.Second)
	defer cancel()
	_, _ = run(sctx, "bluetoothctl", "connect", address)
}

func disconnect(ctx context.Context, run runFunc, address string) {
	sctx, cancel := step(ctx, 10*time.Second)
	defer cancel()
	_, _ = run(sctx, "bluetoothctl", "disconnect", address)
}

func remove(ctx context.Context, run runFunc, address string) {
	sctx, cancel := step(ctx, 10*time.Second)
	defer cancel()
	_, _ = run(sctx, "bluetoothctl", "remove", address)
}
