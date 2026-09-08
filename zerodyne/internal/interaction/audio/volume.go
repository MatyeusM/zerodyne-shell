package audio

import (
	"context"
	"fmt"
	"regexp"
	"strconv"
	"time"

	"zerodyne/internal/interaction"
	zer_audio "zerodyne/internal/monitor/audio"
)

// VolumeHandler implements "audio-volume <raise|lower|mute-toggle|+N|-N>",
// ported from omarchy-audio-output-volumne. raise is +5, lower is -5. Any
// +N clears mute (unmutes-via-raise, like the script); writes clamp to
// 0-100 while reads stay raw.
type VolumeHandler struct {
	run runFunc
}

// NewVolumeHandler builds the handler with live system access.
func NewVolumeHandler() *VolumeHandler { return &VolumeHandler{run: liveRun} }

// NewVolumeHandlerWithRunner builds the handler with a stub runner (tests).
func NewVolumeHandlerWithRunner(run runFunc) *VolumeHandler { return &VolumeHandler{run: run} }

// Name implements interaction.Handler.
func (h *VolumeHandler) Name() string { return "audio-volume" }

// Usage implements interaction.Handler.
func (h *VolumeHandler) Usage() string { return "audio-volume <raise|lower|mute-toggle|+N|-N>" }

var deltaRe = regexp.MustCompile(`^([+-])(\d+)$`)

// Exec implements interaction.Handler.
func (h *VolumeHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 1 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	action := args[0]
	switch action {
	case "raise":
		action = "+5"
	case "lower":
		action = "-5"
	}
	sink, err := volumeSink(ctx, h.run)
	if err != nil {
		return interaction.Result{}, err
	}
	if action == "mute-toggle" {
		sctx, cancel := step(ctx, 5*time.Second)
		defer cancel()
		if _, err := h.run(sctx, "pactl", "set-sink-mute", sink, "toggle"); err != nil {
			return interaction.Result{}, fmt.Errorf("audio-volume: %v", err)
		}
		return interaction.Result{Message: "mute toggled"}, nil
	}
	m := deltaRe.FindStringSubmatch(action)
	if m == nil {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	stepN, _ := strconv.Atoi(m[2])
	sctx, cancel := step(ctx, 5*time.Second)
	defer cancel()
	run := zer_audio.RunFunc(h.run)
	current, err := zer_audio.SinkVolumePct(sctx, run, sink)
	if err != nil {
		return interaction.Result{}, err
	}
	next := current + stepN
	if m[1] == "-" {
		next = current - stepN
	}
	next = clampPct(next)
	// Unmute-via-raise: the level moves on the real sink either way.
	if _, err := h.run(sctx, "pactl", "set-sink-mute", sink, "0"); err != nil {
		return interaction.Result{}, fmt.Errorf("audio-volume: %v", err)
	}
	if _, err := h.run(sctx, "pactl", "set-sink-volume", sink, strconv.Itoa(next)+"%"); err != nil {
		return interaction.Result{}, fmt.Errorf("audio-volume: %v", err)
	}
	return interaction.Result{Message: fmt.Sprintf("%d%%", next)}, nil
}
