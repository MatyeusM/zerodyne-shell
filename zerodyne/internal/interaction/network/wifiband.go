// Package wifiband implements the "wifi-band" interaction handler, ported
// from services/network-band.sh.
//
// Query (`zerodyne exec wifi-band`) reports the current band, the bands the
// SSID is reachable on, and the pinned band of the NM profile. Setting
// (`zerodyne exec wifi-band 5|auto|...`) pins 802-11-wireless.band on the
// active profile and reassociates, reverting on failure like the script.
package network

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"sort"
	"strings"
	"time"

	"zerodyne/internal/interaction"
	netmon "zerodyne/internal/monitor/network"
)

// Handler is the "wifi-band" interaction handler.
type Handler struct {
	run runFunc
}

type runFunc func(ctx context.Context, name string, args ...string) (string, error)

// NewHandler builds the handler with live system access.
func NewHandler() *Handler {
	return &Handler{run: func(ctx context.Context, name string, args ...string) (string, error) {
		ctx, cancel := context.WithTimeout(ctx, 15*time.Second)
		defer cancel()
		cmd := exec.CommandContext(ctx, name, args...)
		if name == "nmcli" {
			// Pin the locale: nmcli translates state words such as
			// "connected", which would silently stop matching.
			cmd.Env = append(os.Environ(), "LC_ALL=C")
		}
		out, err := cmd.Output()
		return strings.TrimRight(string(out), "\n"), err
	}}
}

// NewHandlerWithRunner builds the handler with a stub runner (tests).
func NewHandlerWithRunner(run runFunc) *Handler { return &Handler{run: run} }

// Name implements interaction.Handler.
func (h *Handler) Name() string { return "wifi-band" }

// Usage implements interaction.Handler.
func (h *Handler) Usage() string { return "wifi-band [auto|2.4|5|6|<freq-mhz>]" }

// Status is the query result: current band, reachable bands, pinned band.
type Status struct {
	Band      string   `json:"band"`
	Available []string `json:"available"`
	Selected  string   `json:"selected"`
}

// Exec implements interaction.Handler.
func (h *Handler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) == 0 {
		st, err := h.status(ctx)
		if err != nil {
			return interaction.Result{}, err
		}
		return interaction.Result{Data: map[string]any{
			"band":      st.Band,
			"available": st.Available,
			"selected":  st.Selected,
		}}, nil
	}
	if len(args) > 1 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	band, ok := netmon.NormalizeBandTarget(args[0])
	if !ok {
		return interaction.Result{}, fmt.Errorf("invalid band %q: usage: %s", args[0], h.Usage())
	}
	if err := h.setBand(ctx, band); err != nil {
		return interaction.Result{}, err
	}
	return interaction.Result{Message: "band set to " + band}, nil
}

func (h *Handler) status(ctx context.Context) (Status, error) {
	dev, err := h.wifiDevice(ctx)
	if err != nil || dev == "" {
		return Status{}, fmt.Errorf("wifi-band: no connected Wi-Fi device")
	}
	link, err := h.run(ctx, "iw", "dev", dev, "link")
	if err != nil || !strings.Contains(link, "SSID:") {
		return Status{}, fmt.Errorf("wifi-band: no active Wi-Fi link")
	}
	ssid := iwSSID(link)
	freq := iwFreq(link)
	band, _ := netmon.BandForFreq(freq)
	available := h.availableBands(ctx, dev, ssid, band)
	profile, _ := h.profile(ctx, dev)
	selected := netmon.BandAuto
	if profile != "" {
		if v, err := h.nmGet(ctx, "802-11-wireless.band", "connection", "show", profile); err == nil {
			selected = netmon.BandFromNm(v)
		}
	}
	return Status{Band: band, Available: available, Selected: selected}, nil
}

func (h *Handler) setBand(ctx context.Context, target string) error {
	dev, err := h.wifiDevice(ctx)
	if err != nil || dev == "" {
		return fmt.Errorf("wifi-band: no connected Wi-Fi device")
	}
	profile, err := h.profile(ctx, dev)
	if err != nil || profile == "" {
		return fmt.Errorf("wifi-band: no active Wi-Fi connection profile")
	}
	desired := ""
	if target != netmon.BandAuto {
		link, _ := h.run(ctx, "iw", "dev", dev, "link")
		ssid := iwSSID(link)
		cur, _ := netmon.BandForFreq(iwFreq(link))
		if !contains(h.availableBands(ctx, dev, ssid, cur), target) {
			return fmt.Errorf("wifi-band: %sGHz is not available on this network", target)
		}
		nm, ok := netmon.NmBandFor(target)
		if !ok {
			return fmt.Errorf("wifi-band: invalid band %q", target)
		}
		desired = nm
	}
	previous, _ := h.nmGet(ctx, "802-11-wireless.band", "connection", "show", profile)
	if previous == desired {
		return nil
	}
	if _, err := h.run(ctx, "nmcli", "connection", "modify", profile, "802-11-wireless.band", desired); err != nil {
		return fmt.Errorf("wifi-band: modify profile: %w", err)
	}
	// A band change only takes effect on reassociation. Revert rather than
	// strand the machine offline.
	if _, err := h.run(ctx, "nmcli", "connection", "up", profile); err != nil {
		_, _ = h.run(ctx, "nmcli", "connection", "modify", profile, "802-11-wireless.band", previous)
		_, _ = h.run(ctx, "nmcli", "connection", "up", profile)
		return fmt.Errorf("wifi-band: could not connect on %s; reverted to previous band", target)
	}
	return nil
}

func (h *Handler) wifiDevice(ctx context.Context) (string, error) {
	out, err := h.nmGet(ctx, "DEVICE,TYPE,STATE", "device", "status")
	if err != nil {
		return "", err
	}
	for _, line := range strings.Split(out, "\n") {
		parts := strings.SplitN(line, ":", 3)
		if len(parts) == 3 && parts[1] == "wifi" && strings.HasPrefix(parts[2], "connected") {
			return parts[0], nil
		}
	}
	return "", nil
}

func (h *Handler) profile(ctx context.Context, dev string) (string, error) {
	return h.nmGet(ctx, "GENERAL.CONNECTION", "device", "show", dev)
}

// nmGet runs `LC_ALL=C nmcli -e no -g ...`: -e no keeps ':'/'\' in values
// unescaped so SSIDs match iw's raw form.
func (h *Handler) nmGet(ctx context.Context, field string, args ...string) (string, error) {
	full := append([]string{"-e", "no", "-g", field}, args...)
	out, err := h.run(ctx, "nmcli", full...)
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(out), nil
}

// availableBands lists every band the SSID is reachable on (low to high),
// always including the current one, from NM's cache (--rescan no).
func (h *Handler) availableBands(ctx context.Context, dev, ssid, current string) []string {
	set := map[string]bool{}
	if current != "" {
		set[current] = true
	}
	out, err := h.nmGet(ctx, "FREQ,SSID", "dev", "wifi", "list", "ifname", dev, "--rescan", "no")
	if err == nil {
		for _, line := range strings.Split(out, "\n") {
			idx := strings.Index(line, ":")
			if idx < 0 {
				continue
			}
			// SSID is everything after the first ':' reassembled verbatim
			// so names containing ':' match iw's raw form.
			freq, name := line[:idx], line[idx+1:]
			if name != ssid {
				continue
			}
			if b, ok := netmon.BandForFreq(strings.TrimSpace(freq)); ok {
				set[b] = true
			}
		}
	}
	return sortedKeys(set)
}

func sortedKeys(set map[string]bool) []string {
	// Numeric-aware order: 2.4 < 5 < 6.
	order := map[string]int{"2.4": 0, "5": 1, "6": 2}
	out := make([]string, 0, len(set))
	for b := range set {
		out = append(out, b)
	}
	sort.Slice(out, func(i, j int) bool { return order[out[i]] < order[out[j]] })
	return out
}

func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}

func iwSSID(link string) string {
	for _, line := range strings.Split(link, "\n") {
		if i := strings.Index(line, "SSID:"); i >= 0 {
			return strings.TrimSpace(line[i+len("SSID:"):])
		}
	}
	return ""
}

func iwFreq(link string) string {
	for _, line := range strings.Split(link, "\n") {
		line = strings.TrimSpace(line)
		if v, ok := strings.CutPrefix(line, "freq:"); ok {
			return strings.TrimSpace(v)
		}
	}
	return ""
}
