package audio

import (
	"context"
	"fmt"
	"strconv"
	"time"

	"zerodyne/internal/interaction"
	zer_audio "zerodyne/internal/monitor/audio"
)

// InputMuteHandler implements "audio-input-mute [toggle|on|off|+N|-N]",
// ported from omarchy-audio-input-mute (`wpctl set-mute
// @DEFAULT_AUDIO_SOURCE@`; mute state drives laptop mic LEDs wpctl-side).
// The +N/-N form is a small extension beyond the script so the panel
// INPUT slider has a write path; toggle|on|off behave exactly as ported.
type InputMuteHandler struct {
	run runFunc
}

// NewInputMuteHandler builds the handler with live system access.
func NewInputMuteHandler() *InputMuteHandler { return &InputMuteHandler{run: liveRun} }

// NewInputMuteHandlerWithRunner builds the handler with a stub runner (tests).
func NewInputMuteHandlerWithRunner(run runFunc) *InputMuteHandler {
	return &InputMuteHandler{run: run}
}

// Name implements interaction.Handler.
func (h *InputMuteHandler) Name() string { return "audio-input-mute" }

// Usage implements interaction.Handler.
func (h *InputMuteHandler) Usage() string { return "audio-input-mute [toggle|on|off|+N|-N]" }

const defaultSource = "@DEFAULT_AUDIO_SOURCE@"

// Exec implements interaction.Handler.
func (h *InputMuteHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	action := "toggle"
	if len(args) > 1 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	if len(args) == 1 {
		action = args[0]
	}
	sctx, cancel := step(ctx, 5*time.Second)
	defer cancel()
	switch action {
	case "toggle":
		if _, err := h.run(sctx, "wpctl", "set-mute", defaultSource, "toggle"); err != nil {
			return interaction.Result{}, fmt.Errorf("audio-input-mute: %v", err)
		}
		return interaction.Result{Message: "microphone toggled"}, nil
	case "on":
		if _, err := h.run(sctx, "wpctl", "set-mute", defaultSource, "1"); err != nil {
			return interaction.Result{}, fmt.Errorf("audio-input-mute: %v", err)
		}
		return interaction.Result{Message: "microphone muted"}, nil
	case "off":
		if _, err := h.run(sctx, "wpctl", "set-mute", defaultSource, "0"); err != nil {
			return interaction.Result{}, fmt.Errorf("audio-input-mute: %v", err)
		}
		return interaction.Result{Message: "microphone on"}, nil
	}
	m := deltaRe.FindStringSubmatch(action)
	if m == nil {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	stepN, _ := strconv.Atoi(m[2])
	out, err := h.run(sctx, "wpctl", "get-volume", defaultSource)
	if err != nil {
		return interaction.Result{}, fmt.Errorf("audio-input-mute: %v", err)
	}
	current, _ := zer_audio.ParseWpctlVolume(out)
	next := current + stepN
	if m[1] == "-" {
		next = current - stepN
	}
	next = clampPct(next)
	if _, err := h.run(sctx, "wpctl", "set-volume", defaultSource, strconv.Itoa(next)+"%"); err != nil {
		return interaction.Result{}, fmt.Errorf("audio-input-mute: %v", err)
	}
	return interaction.Result{Message: fmt.Sprintf("%d%%", next)}, nil
}
