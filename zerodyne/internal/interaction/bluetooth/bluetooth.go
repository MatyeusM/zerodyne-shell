// Package bluetooth implements the bluetooth interaction handlers, ported
// from omarchy-bluetooth-power and omarchy-bluetooth-device.
//
// Monitoring (adapter state, device list) lives in QML via
// Quickshell.Bluetooth; only the state-changing sequencing lives here so
// the shell is not coupled to omarchy's distribution scripts. Like the
// scripts, device operations are best-effort: BlueZ is polled by the UI
// afterwards, so a slow pairing reports through state, not the exit code.
package bluetooth

import (
	"context"
	"os/exec"
	"strings"
	"time"
)

type runFunc func(ctx context.Context, name string, args ...string) (string, error)

// liveRun executes system commands with a generous ceiling: pairing can
// take tens of seconds, and the daemon request context (30s) bounds the
// worst case anyway. Individual steps derive shorter timeouts from ctx.
func liveRun(ctx context.Context, name string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, 60*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, name, args...)
	out, err := cmd.CombinedOutput()
	return strings.TrimRight(string(out), "\n"), err
}

// step derives a bounded context for one sequencing step, mirroring the
// scripts' `timeout Ns` wrappers. Stubs ignore the deadline.
func step(ctx context.Context, d time.Duration) (context.Context, context.CancelFunc) {
	return context.WithTimeout(ctx, d)
}
