// Package powerprofile implements "power-profile-set <profile>", switching
// the power-profiles-daemon profile. State is observed via the
// power-profile monitor; this only changes it.
package powerprofile

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"zerodyne/internal/interaction"
	zer_profile "zerodyne/internal/monitor/powerprofile"
)

type runFunc func(ctx context.Context, name string, args ...string) (string, error)

// liveRun executes system commands with a ceiling well above a
// single-shot set call.
func liveRun(ctx context.Context, name string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, 15*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, name, args...)
	out, err := cmd.CombinedOutput()
	return strings.TrimRight(string(out), "\n"), err
}

// step derives a bounded context for one sequencing step. Stubs ignore
// the deadline.
func step(ctx context.Context, d time.Duration) (context.Context, context.CancelFunc) {
	return context.WithTimeout(ctx, d)
}

// SetHandler implements "power-profile-set <profile>".
type SetHandler struct {
	run runFunc
}

// NewSetHandler builds the handler with live system access.
func NewSetHandler() *SetHandler { return &SetHandler{run: liveRun} }

// NewSetHandlerWithRunner builds the handler with a stub runner (tests).
func NewSetHandlerWithRunner(run runFunc) *SetHandler { return &SetHandler{run: run} }

// Name implements interaction.Handler.
func (h *SetHandler) Name() string { return "power-profile-set" }

// Usage implements interaction.Handler.
func (h *SetHandler) Usage() string { return "power-profile-set <profile>" }

// Exec implements interaction.Handler.
func (h *SetHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 1 || !zer_profile.ValidProfile(args[0]) {
		return interaction.Result{}, fmt.Errorf("usage: %s (known: %s)", h.Usage(), strings.Join(zer_profile.KnownProfiles, ", "))
	}
	sctx, cancel := step(ctx, 10*time.Second)
	defer cancel()
	if out, err := h.run(sctx, "powerprofilesctl", "set", args[0]); err != nil {
		return interaction.Result{}, fmt.Errorf("power-profile-set: %s", firstLine(out))
	}
	return interaction.Result{Message: args[0]}, nil
}

func firstLine(s string) string {
	if i := strings.Index(s, "\n"); i >= 0 {
		return strings.TrimSpace(s[:i])
	}
	return strings.TrimSpace(s)
}
