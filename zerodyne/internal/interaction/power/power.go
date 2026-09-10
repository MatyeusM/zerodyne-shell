// Package power implements the session power handler, ported from the
// user's rofi power menu (~/.config/rofi/scripts/power-menu.sh): same
// four commands, minus suspend (no sleep on this shell).
//
//	logout   — hyprctl dispatch 'hl.dsp.exit()' (dispatch takes lua)
//	lock     — hyprlock
//	restart  — systemctl reboot
//	shutdown — systemctl poweroff
//
// All four are fire-and-forget: the session ends (or the screen locks),
// so there is no state for QML to re-poll. Only usage errors fail.
package power

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"zerodyne/internal/interaction"
)

type runFunc func(ctx context.Context, name string, args ...string) (string, error)

// liveRun executes system commands with a ceiling well above any
// single-shot power call.
func liveRun(ctx context.Context, name string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, 15*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, name, args...)
	out, err := cmd.CombinedOutput()
	return strings.TrimRight(string(out), "\n"), err
}

// step derives a bounded context for one sequencing step, mirroring the
// scripts' `timeout N` wrappers. Stubs ignore the deadline.
func step(ctx context.Context, d time.Duration) (context.Context, context.CancelFunc) {
	return context.WithTimeout(ctx, d)
}

// Handler implements "power <logout|lock|restart|shutdown>".
type Handler struct {
	run runFunc
}

// NewHandler builds the handler with live system access.
func NewHandler() *Handler { return &Handler{run: liveRun} }

// NewHandlerWithRunner builds the handler with a stub runner (tests).
func NewHandlerWithRunner(run runFunc) *Handler { return &Handler{run: run} }

// Name implements interaction.Handler.
func (h *Handler) Name() string { return "power" }

// Usage implements interaction.Handler.
func (h *Handler) Usage() string { return "power <logout|lock|restart|shutdown>" }

// Exec implements interaction.Handler.
func (h *Handler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 1 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	sctx, cancel := step(ctx, 10*time.Second)
	defer cancel()
	switch args[0] {
	case "logout":
		if _, err := h.run(sctx, "hyprctl", "dispatch", "hl.dsp.exit()"); err != nil {
			return interaction.Result{}, fmt.Errorf("power: %v", err)
		}
		return interaction.Result{Message: "logging out"}, nil
	case "lock":
		if _, err := h.run(sctx, "hyprlock"); err != nil {
			return interaction.Result{}, fmt.Errorf("power: %v", err)
		}
		return interaction.Result{Message: "locked"}, nil
	case "restart":
		if _, err := h.run(sctx, "systemctl", "reboot"); err != nil {
			return interaction.Result{}, fmt.Errorf("power: %v", err)
		}
		return interaction.Result{Message: "restarting"}, nil
	case "shutdown":
		if _, err := h.run(sctx, "systemctl", "poweroff"); err != nil {
			return interaction.Result{}, fmt.Errorf("power: %v", err)
		}
		return interaction.Result{Message: "shutting down"}, nil
	default:
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
}
