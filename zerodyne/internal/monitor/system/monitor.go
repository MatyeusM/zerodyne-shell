// Package system implements the system monitor: host-level facts that fit
// neither network nor performance (hostname, uptime). Extension point for
// power profiles and other host state later.
package system

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"

	"zerodyne/internal/monitor"
)

// Snapshot is the typed system state.
type Snapshot struct {
	Hostname   string `json:"hostname"`
	UptimeSecs int64  `json:"uptime_secs"`
}

// Monitor is the registered "system" monitor.
type Monitor struct {
	monitor.Base
}

// NewMonitor builds the system monitor.
func NewMonitor() *Monitor { return &Monitor{} }

// Name implements monitor.Monitor.
func (m *Monitor) Name() string { return "system" }

// Snapshot implements monitor.Monitor.
func (m *Monitor) Snapshot(ctx context.Context) (any, error) {
	_ = ctx
	host, err := os.Hostname()
	if err != nil {
		host = ""
	}
	up, err := readUptime()
	if err != nil {
		return nil, err
	}
	return Snapshot{Hostname: host, UptimeSecs: up}, nil
}

func readUptime() (int64, error) {
	b, err := os.ReadFile("/proc/uptime")
	if err != nil {
		return 0, fmt.Errorf("system: /proc/uptime: %w", err)
	}
	f, err := strconv.ParseFloat(strings.Fields(string(b))[0], 64)
	if err != nil {
		return 0, fmt.Errorf("system: parse /proc/uptime: %w", err)
	}
	return int64(f), nil
}
