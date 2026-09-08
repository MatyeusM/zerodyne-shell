package network

import (
	"context"
	"fmt"

	"zerodyne/internal/interaction"
	netmon "zerodyne/internal/monitor/network"
)

// ScanHandler implements "wifi-scan [rescan]": forced synchronous refresh,
// mostly for CLI use. The QML panel reads the background cache via the
// wifi monitor instead, so opening it never waits on the radio.
type ScanHandler struct {
	run runFunc
}

// NewScanHandler builds the handler with live system access.
func NewScanHandler() *ScanHandler { return &ScanHandler{run: liveRun} }

// NewScanHandlerWithRunner builds the handler with a stub runner (tests).
func NewScanHandlerWithRunner(run runFunc) *ScanHandler { return &ScanHandler{run: run} }

// Name implements interaction.Handler.
func (h *ScanHandler) Name() string { return "wifi-scan" }

// Usage implements interaction.Handler.
func (h *ScanHandler) Usage() string { return "wifi-scan [rescan]" }

// Exec implements interaction.Handler.
func (h *ScanHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) > 1 || (len(args) == 1 && args[0] != "rescan") {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	dev := netmon.ScanDevice(ctx, netmon.RunFunc(h.run))
	if dev == "" {
		return interaction.Result{}, fmt.Errorf("wifi-scan: no wifi device")
	}
	nets, err := netmon.ScanVisible(ctx, netmon.RunFunc(h.run), dev, len(args) == 1)
	if err != nil {
		return interaction.Result{}, fmt.Errorf("wifi-scan: %w", err)
	}
	return interaction.Result{Data: map[string]any{
		"device":   dev,
		"networks": nets,
	}}, nil
}
