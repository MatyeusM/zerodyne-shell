// Package audio implements the audio interaction handlers, ported from
// omarchy-scripts/omarchy-audio-output-volumne (typo intentional in the
// source), omarchy-audio-input-mute, omarchy-audio-output-set-default,
// omarchy-audio-input-set-default, and omarchy-audio-output-switch.
//
// Monitoring (levels, device lists) lives in the audio monitor behind
// `status audio`; only the state-changing sequencing lives here. All five
// ops are single-shot pactl/wpctl calls well under the daemon's 30s
// request cap, so no best-effort polling pattern is needed.
//
// Dropped from the scripts (no equivalents here): the 250ms debounce
// file (single exec per gesture), omarchy-osd calls (no OSD; the switch
// op returns its data for the panel instead, and scroll feedback is the
// indicator re-polling), omarchy-brightness-keyboard-mute, and the
// fronted-sink tuning dependency.
package audio

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"

	zer_audio "zerodyne/internal/monitor/audio"
)

type runFunc func(ctx context.Context, name string, args ...string) (string, error)

// liveRun executes system commands with a ceiling well above any
// single-shot pactl/wpctl call. Individual steps derive shorter timeouts
// from ctx.
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

// funcRunner adapts a runFunc to the monitor audio Runner so handlers
// share the monitor's resolution/parsing helpers.
type funcRunner struct{ run runFunc }

func (f funcRunner) Run(ctx context.Context, name string, args ...string) (string, error) {
	return f.run(ctx, name, args...)
}

// collector builds a monitor collector over the handler's runner.
func collector(run runFunc) *zer_audio.Collector {
	c := zer_audio.NewCollector(funcRunner{run})
	c.Has = func(string) bool { return true }
	return c
}

// volumeSink resolves the sink loudness lives on (through any DSP sink).
func volumeSink(ctx context.Context, run runFunc) (string, error) {
	sctx, cancel := step(ctx, 5*time.Second)
	defer cancel()
	def := zer_audio.DefaultSink(sctx, zer_audio.RunFunc(run))
	if def == "" {
		return "", errNoSink()
	}
	return zer_audio.ResolveVolumeSink(sctx, zer_audio.RunFunc(run), def), nil
}

func errNoSink() error { return fmt.Errorf("audio-volume: no audio sink") }

func clampPct(v int) int {
	if v < 0 {
		return 0
	}
	if v > 100 {
		return 100
	}
	return v
}
