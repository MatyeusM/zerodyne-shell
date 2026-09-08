package gpu

import (
	"context"
	"time"

	"zerodyne/internal/monitor"
)

// Monitor is the registered "gpu" monitor.
type Monitor struct {
	monitor.Base
	collector *Collector
}

// NewMonitor builds the GPU monitor with live system access.
func NewMonitor() *Monitor {
	return &Monitor{collector: NewCollector(DefaultRunner{})}
}

// NewMonitorWithCollector builds the monitor with a stub collector (tests).
func NewMonitorWithCollector(c *Collector) *Monitor {
	return &Monitor{collector: c}
}

// Name implements monitor.Monitor.
func (m *Monitor) Name() string { return "gpu" }

// Snapshot implements monitor.Monitor.
func (m *Monitor) Snapshot(ctx context.Context) (any, error) {
	ctx, cancel := context.WithTimeout(ctx, 8*time.Second)
	defer cancel()
	return m.collector.Collect(ctx)
}
