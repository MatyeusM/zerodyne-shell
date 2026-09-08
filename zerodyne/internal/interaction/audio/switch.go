package audio

import (
	"context"
	"fmt"
	"strconv"
	"time"

	"zerodyne/internal/interaction"
	zer_audio "zerodyne/internal/monitor/audio"
)

// OutputSwitchHandler implements "audio-output-switch [next|previous]"
// (default next), ported from omarchy-audio-output-switch: cycle over
// available sinks, preserving the new sink's mute state. The OSD call
// becomes data: Result.Data carries the new sink so the panel can show
// it.
type OutputSwitchHandler struct {
	run runFunc
}

// NewOutputSwitchHandler builds the handler with live system access.
func NewOutputSwitchHandler() *OutputSwitchHandler { return &OutputSwitchHandler{run: liveRun} }

// NewOutputSwitchHandlerWithRunner builds the handler with a stub runner (tests).
func NewOutputSwitchHandlerWithRunner(run runFunc) *OutputSwitchHandler {
	return &OutputSwitchHandler{run: run}
}

// Name implements interaction.Handler.
func (h *OutputSwitchHandler) Name() string { return "audio-output-switch" }

// Usage implements interaction.Handler.
func (h *OutputSwitchHandler) Usage() string { return "audio-output-switch [next|previous]" }

// Exec implements interaction.Handler.
func (h *OutputSwitchHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	dir := "next"
	if len(args) > 1 || (len(args) == 1 && args[0] != "next" && args[0] != "previous") {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	if len(args) == 1 {
		dir = args[0]
	}
	sctx, cancel := step(ctx, 10*time.Second)
	defer cancel()
	c := collector(h.run)
	sinks, defName, err := c.ListSinks(sctx)
	if err != nil {
		return interaction.Result{}, err
	}
	var avail []zer_audio.SinkEntry
	for _, s := range sinks {
		if s.Available {
			avail = append(avail, s)
		}
	}
	if len(avail) == 0 {
		return interaction.Result{}, fmt.Errorf("audio-output-switch: no audio devices found")
	}
	idx := 0
	for i, s := range avail {
		if s.Name == defName {
			idx = i
			break
		}
	}
	next := idx
	if dir == "next" {
		next = (idx + 1) % len(avail)
	} else {
		next = (idx - 1 + len(avail)) % len(avail)
	}
	target := avail[next]
	if target.Name != defName {
		if err := applyOutputDefault(sctx, h.run, strconv.Itoa(target.Index), target.Name); err != nil {
			return interaction.Result{}, err
		}
	}
	// Read the level from whichever sink actually carries it (a tuning
	// sink sits at fixed 100% unmuted while loudness lives beneath).
	run := zer_audio.RunFunc(h.run)
	eff := zer_audio.ResolveVolumeSink(sctx, run, target.Name)
	vol, _ := zer_audio.SinkVolumePct(sctx, run, eff)
	muted, _ := zer_audio.SinkMuted(sctx, run, eff)
	return interaction.Result{
		Message: target.Description,
		Data: map[string]any{
			"name":        target.Name,
			"description": target.Description,
			"volume_pct":  vol,
			"muted":       muted,
		},
	}, nil
}
