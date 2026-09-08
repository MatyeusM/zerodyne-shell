// Package audio implements the audio monitor: the typed form of
// omarchy-scripts/omarchy-audio-output-sink,
// omarchy-audio-sink-availability, and the volume/mute reads in
// omarchy-audio-output-volumne and omarchy-audio-input-mute, served as
// JSON. QML polls `status audio` and displays; it never shells out and
// never imports Pipewire.
//
// Snapshot sources (ports of the scripts' reads):
//
//	default sink + volume + mute — `pactl get-default-sink`,
//	  `pactl get-sink-volume <sink>` (first % token on line 1),
//	  `pactl get-sink-mute <sink>` (*yes)
//	DSP resolution (omarchy-audio-output-sink): when the default sink is
//	  not alsa_*, follow `pactl list sink-inputs` down to the physical
//	  sink underneath. The indicator/scroll/keys operate on that sink.
//	Sink list (output-switch + sink-availability filters): `pactl -f json
//	  list sinks`, keeping sinks with no ports or any available port.
//	Sources: `pactl get-default-source`, `pactl -f json list sources`;
//	  input volume/mute via `wpctl get-volume @DEFAULT_AUDIO_SOURCE@`.
//	Classification per sink/source: a substring ladder reimplemented
//	  from omarchy Model.js isHeadphones/sinkGlyph/sourceGlyph (order
//	  matters: headphone before phone).
//
// System commands go through the Runner boundary so tests can substitute
// a fake. Tools pactl/wpctl are added to caps.Tools; without them the
// snapshot errors and QML renders the degraded (muted/grey) path.
package audio

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
	// Timeout caps each command. Zero means 3s (pactl is fast; unlike NM
	// scans no long timeout is needed).
	Timeout time.Duration
}

func (r DefaultRunner) timeout() time.Duration {
	if r.Timeout > 0 {
		return r.Timeout
	}
	return 3 * time.Second
}

// Run executes the command, returning stdout. Stderr is discarded: the
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

// RunFunc matches both Runner.Run and the interaction runFunc shape so
// both layers share the parsing/resolution helpers.
type RunFunc func(ctx context.Context, name string, args ...string) (string, error)

// AsRunFunc adapts a Runner to the RunFunc shape.
func AsRunFunc(r Runner) RunFunc {
	return func(ctx context.Context, name string, args ...string) (string, error) {
		return r.Run(ctx, name, args...)
	}
}
