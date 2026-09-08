package network

import (
	"context"
	"os/exec"
	"strings"
	"time"
)

// Runner executes a system command and returns combined stdout.
// All command execution in this package goes through Runner so tests can
// substitute a fake and native D-Bus code can replace it later.
type Runner interface {
	Run(ctx context.Context, name string, args ...string) (string, error)
}

// DefaultRunner invokes real system commands with a per-command timeout.
type DefaultRunner struct {
	// Timeout caps each command. Zero means 3s.
	Timeout time.Duration
}

func (r DefaultRunner) timeout() time.Duration {
	if r.Timeout > 0 {
		return r.Timeout
	}
	return 3 * time.Second
}

// Run executes the command, returning stdout. Stderr is discarded: most
// probes (nmcli, iw) fail by absence on machines without Wi-Fi, and the
// parsers treat empty output as "unknown".
func (r DefaultRunner) Run(ctx context.Context, name string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, r.timeout())
	defer cancel()
	out, err := exec.CommandContext(ctx, name, args...).Output()
	if err != nil {
		return "", err
	}
	return strings.TrimRight(string(out), "\n"), nil
}

// FuncRunner is a stub Runner for tests.
type FuncRunner struct {
	Fn func(ctx context.Context, name string, args ...string) (string, error)
}

func (f FuncRunner) Run(ctx context.Context, name string, args ...string) (string, error) {
	return f.Fn(ctx, name, args...)
}
