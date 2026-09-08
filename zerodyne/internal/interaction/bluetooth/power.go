package bluetooth

import (
	"context"
	"fmt"
	"strings"
	"time"

	"zerodyne/internal/interaction"
)

// PowerHandler implements "bluetooth-power [on|off|toggle|is-on]",
// ported from omarchy-bluetooth-power.
//
// BlueZ never persists Powered, so the rfkill soft block is the state
// (systemd-rfkill restores it across reboots); unblocking lets bluetoothd
// power the adapter up by itself.
type PowerHandler struct {
	run runFunc
}

// NewPowerHandler builds the handler with live system access.
func NewPowerHandler() *PowerHandler { return &PowerHandler{run: liveRun} }

// NewPowerHandlerWithRunner builds the handler with a stub runner (tests).
func NewPowerHandlerWithRunner(run runFunc) *PowerHandler { return &PowerHandler{run: run} }

// Name implements interaction.Handler.
func (h *PowerHandler) Name() string { return "bluetooth-power" }

// Usage implements interaction.Handler.
func (h *PowerHandler) Usage() string { return "bluetooth-power [on|off|toggle|is-on]" }

// powerWait bounds the whole powered-up wait: probes can each sit on their
// own timeout when D-Bus is wedged, so one deadline covers them all.
const powerWait = 2 * time.Second

// Exec implements interaction.Handler.
func (h *PowerHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 1 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	switch args[0] {
	case "on":
		if err := powerOn(ctx, h.run); err != nil {
			return interaction.Result{}, err
		}
		return interaction.Result{Message: "bluetooth on"}, nil
	case "off":
		// No `power off` to go with this: the block already drops the
		// adapter to Powered: no, and it is the half that survives reboot.
		if _, err := h.run(ctx, "rfkill", "block", "bluetooth"); err != nil {
			return interaction.Result{}, fmt.Errorf("bluetooth-power: %v", err)
		}
		return interaction.Result{Message: "bluetooth off"}, nil
	case "toggle":
		on, err := powered(ctx, h.run)
		if err != nil {
			return interaction.Result{}, err
		}
		if on {
			if _, err := h.run(ctx, "rfkill", "block", "bluetooth"); err != nil {
				return interaction.Result{}, fmt.Errorf("bluetooth-power: %v", err)
			}
			return interaction.Result{Message: "bluetooth off"}, nil
		}
		if err := powerOn(ctx, h.run); err != nil {
			return interaction.Result{}, err
		}
		return interaction.Result{Message: "bluetooth on"}, nil
	case "is-on":
		on, err := powered(ctx, h.run)
		if err != nil {
			return interaction.Result{}, err
		}
		if !on {
			return interaction.Result{}, fmt.Errorf("bluetooth-power: off")
		}
		return interaction.Result{Message: "on"}, nil
	default:
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
}

// controllers lists every bluetooth controller, like the script's awk.
// Any controller counts: the block is all-or-nothing across radios.
func controllers(ctx context.Context, run runFunc) []string {
	sctx, cancel := step(ctx, 5*time.Second)
	defer cancel()
	out, err := run(sctx, "bluetoothctl", "list")
	if err != nil {
		return nil
	}
	var ctrls []string
	for _, line := range strings.Split(out, "\n") {
		fields := strings.Fields(line)
		if len(fields) >= 2 {
			ctrls = append(ctrls, fields[1])
		}
	}
	return ctrls
}

// powered reports whether any controller is up. It never fails: no
// controllers (or wedged D-Bus) simply reads as off.
func powered(ctx context.Context, run runFunc) (bool, error) {
	for _, c := range controllers(ctx, run) {
		sctx, cancel := step(ctx, 5*time.Second)
		out, err := run(sctx, "bluetoothctl", "show", c)
		cancel()
		if err == nil && strings.Contains(out, "Powered: yes") {
			return true, nil
		}
	}
	return false, nil
}

// waitPowered polls until any controller reports up or the deadline hits.
func waitPowered(ctx context.Context, run runFunc) bool {
	deadline := time.Now().Add(powerWait)
	for {
		if on, _ := powered(ctx, run); on {
			return true
		}
		if time.Now().After(deadline) {
			return false
		}
		select {
		case <-ctx.Done():
			return false
		case <-time.After(200 * time.Millisecond):
		}
	}
}

// powerOn unblocks the radios and waits for bluetoothd to power up,
// asking directly before giving up (an adapter powered down without a
// block does not come up on its own).
func powerOn(ctx context.Context, run runFunc) error {
	if _, err := run(ctx, "rfkill", "unblock", "bluetooth"); err != nil {
		return fmt.Errorf("bluetooth-power: %v", err)
	}
	if waitPowered(ctx, run) {
		return nil
	}
	sctx, cancel := step(ctx, 5*time.Second)
	defer cancel()
	_, _ = run(sctx, "bluetoothctl", "power", "on")
	if waitPowered(ctx, run) {
		return nil
	}
	return fmt.Errorf("bluetooth-power: adapter did not come up")
}

// ensurePowered powers the default adapter up when needed, for device
// operations. A plain `power on` fails outright while the rfkill block is
// set, so this goes through the block like every other on-path.
func ensurePowered(ctx context.Context, run runFunc) {
	sctx, cancel := step(ctx, 5*time.Second)
	defer cancel()
	out, err := run(sctx, "bluetoothctl", "show")
	if err == nil && strings.Contains(out, "Powered: yes") {
		return
	}
	_ = powerOn(ctx, run)
}
