package audio

import (
	"context"
	"fmt"
	"strings"
	"time"

	"zerodyne/internal/interaction"
	zer_audio "zerodyne/internal/monitor/audio"
)

// OutputDefaultHandler implements
// "audio-output-default <node-id> <sink-name>", ported from
// omarchy-audio-output-set-default: wpctl + pactl set the default, then
// active app sink-inputs move over. Only real application streams move:
// a DSP filter-chain's own output carries no application.name, and moving
// it would rewire the processing itself (onto headphones, or a cycle);
// EasyEffects stays put for the same reason.
type OutputDefaultHandler struct {
	run runFunc
}

// NewOutputDefaultHandler builds the handler with live system access.
func NewOutputDefaultHandler() *OutputDefaultHandler { return &OutputDefaultHandler{run: liveRun} }

// NewOutputDefaultHandlerWithRunner builds the handler with a stub runner (tests).
func NewOutputDefaultHandlerWithRunner(run runFunc) *OutputDefaultHandler {
	return &OutputDefaultHandler{run: run}
}

// Name implements interaction.Handler.
func (h *OutputDefaultHandler) Name() string { return "audio-output-default" }

// Usage implements interaction.Handler.
func (h *OutputDefaultHandler) Usage() string { return "audio-output-default <node-id> <sink-name>" }

// Exec implements interaction.Handler.
func (h *OutputDefaultHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 2 || args[0] == "" || args[1] == "" {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	if err := applyOutputDefault(ctx, h.run, args[0], args[1]); err != nil {
		return interaction.Result{}, err
	}
	return interaction.Result{Message: "output " + args[1]}, nil
}

// applyOutputDefault is the shared set-default sequencing, also used by
// the output switcher. Stream moves are best-effort (|| true in the
// script): the default is already set, so a racing stream is cosmetic.
func applyOutputDefault(ctx context.Context, run runFunc, nodeID, sinkName string) error {
	sctx, cancel := step(ctx, 10*time.Second)
	defer cancel()
	_, _ = run(sctx, "wpctl", "set-default", nodeID)
	_, _ = run(sctx, "pactl", "set-default-sink", sinkName)
	out, err := run(sctx, "pactl", "list", "sink-inputs")
	if err != nil {
		return nil
	}
	for _, id := range zer_audio.AppSinkInputIDs(out) {
		_, _ = run(sctx, "pactl", "move-sink-input", id, sinkName)
	}
	return nil
}

// InputDefaultHandler implements
// "audio-input-default <node-id> <source-name>", ported from
// omarchy-audio-input-set-default: wpctl + pactl set the default, then
// every active source-output moves over.
type InputDefaultHandler struct {
	run runFunc
}

// NewInputDefaultHandler builds the handler with live system access.
func NewInputDefaultHandler() *InputDefaultHandler { return &InputDefaultHandler{run: liveRun} }

// NewInputDefaultHandlerWithRunner builds the handler with a stub runner (tests).
func NewInputDefaultHandlerWithRunner(run runFunc) *InputDefaultHandler {
	return &InputDefaultHandler{run: run}
}

// Name implements interaction.Handler.
func (h *InputDefaultHandler) Name() string { return "audio-input-default" }

// Usage implements interaction.Handler.
func (h *InputDefaultHandler) Usage() string { return "audio-input-default <node-id> <source-name>" }

// Exec implements interaction.Handler.
func (h *InputDefaultHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 2 || args[0] == "" || args[1] == "" {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	sctx, cancel := step(ctx, 10*time.Second)
	defer cancel()
	_, _ = h.run(sctx, "wpctl", "set-default", args[0])
	_, _ = h.run(sctx, "pactl", "set-default-source", args[1])
	if out, err := h.run(sctx, "pactl", "list", "short", "source-outputs"); err == nil {
		for _, line := range strings.Split(out, "\n") {
			if f := strings.Fields(line); len(f) >= 1 {
				_, _ = h.run(sctx, "pactl", "move-source-output", f[0], args[1])
			}
		}
	}
	return interaction.Result{Message: "input " + args[1]}, nil
}
