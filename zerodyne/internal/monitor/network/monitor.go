// Package network implements the network monitor.
//
// This is the single network status: the typed form of
// services/network-status.sh --verbose, served as JSON. The script's
// non-verbose mode is gone — its derived values (kind label, header
// frequency) follow trivially from Type/SSID/Iface/FreqMhz.
//
// Specification: services/network-status.sh, services/network-band.sh
// status query, and modules/omonetwork/NetworkModel.js + Network.qml
// (polling intervals, derived values).
//
// Collected (mirrors the script, field for field):
//
//	default route device  — `ip route get 1.1.1.1` (script's internet_probe)
//	wifi vs ethernet      — /sys/class/net/<dev>/wireless presence
//	wifi ssid/signal      — nmcli GENERAL.CONNECTION / wifi list signal
//	wifi freq             — `iw dev <dev> link`
//	ip/prefix/gateway     — `ip -j route get` + `ip -j addr show`
//	rx/tx counters        — /sys/class/net counters
//	ssid/signal/freq/bitrate — `iw dev <dev> link`
//	ethernet speed/duplex — sysfs
//	router/internet ping  — parallel `ping -c1 -W1`
//
// Aggregated in Go (previously NetworkModel.js in QML): per-interface byte
// rates (throughputState) and ping history averages + packet loss
// (pingLatencyState, 24-sample window, 5-sample average).
//
// Capability probing (internal/caps) replaces cmd-present.sh: tools absent
// from PATH are skipped instead of forked-and-failed, so minimal systems
// get fast degraded snapshots and QML can render "unavailable" from daemon
// info instead of polling a dead backend.
//
// Polling intervals currently in QML: details every 1500ms, band status
// every 4000ms, only while the panel is open. The monitor is on-demand
// today; background caching can be added behind Start without changing the
// snapshot shape.
//
// System commands are invoked only through the Runner boundary so native
// NetworkManager D-Bus access can replace nmcli/iw later without touching
// callers.
package network

import (
	"context"
	"time"

	"zerodyne/internal/monitor"
)

// Monitor is the registered "network" monitor.
type Monitor struct {
	monitor.Base
	collector *Collector
	tracker   *ThroughputTracker
	pings     *PingTracker
}

// NewMonitor builds the network monitor with live system access.
func NewMonitor() *Monitor {
	return &Monitor{
		collector: NewCollector(DefaultRunner{}),
		tracker:   NewThroughputTracker(),
		pings:     NewPingTracker(),
	}
}

// Name implements monitor.Monitor.
func (m *Monitor) Name() string { return "network" }

// Snapshot implements monitor.Monitor.
func (m *Monitor) Snapshot(ctx context.Context) (any, error) {
	ctx, cancel := context.WithTimeout(ctx, 8*time.Second)
	defer cancel()
	st, err := m.collector.Collect(ctx)
	if err != nil {
		return nil, err
	}
	down, up := m.tracker.Update(st.Iface, st.RxBytes, st.TxBytes, time.Now())
	st.DownloadBps = down
	st.UploadBps = up
	stats := m.pings.Update(st.Iface, st.RouterPingMs, st.InternetPingMs)
	st.RouterPingAvgMs = stats.RouterAvg
	st.InternetPingAvgMs = stats.InternetAvg
	st.InternetLossPct = stats.InternetLossPct
	st.PingSamples = stats.Samples
	return st, nil
}
