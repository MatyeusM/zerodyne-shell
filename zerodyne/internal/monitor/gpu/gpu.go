// Package gpu implements the GPU monitor: utilization, temperature,
// power, and VRAM for the first GPU, parsed from `amd-smi metric` and
// `amd-smi static` JSON (never rocm-smi). QML polls `status gpu` and
// displays; all parsing lives here.
//
// Both calls take ~0.15s, well under the default runner timeout. Values
// may be the string "N/A" (idle engines, missing sensors) and decode to
// zero. The static name lookup is best-effort: metrics are still reported
// when it fails.
package gpu

import (
	"context"
	"os/exec"
	"strings"
	"time"
)

// Runner executes a system command and returns stdout. All command
// execution in this package goes through Runner so tests can substitute
// a fake.
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

// Run executes the command, returning stdout.
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
