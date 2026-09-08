// The wifi monitor: the visible-access-point list with background
// refreshing.
//
// The QML panel must never wait on a radio scan: this monitor rescans
// lazily in the background (real NM scan every ScanInterval) while the
// daemon is active, and snapshots serve the cache instantly. `zerodyne
// exec wifi-scan rescan` still forces a synchronous refresh for CLI use.
package network

import (
	"context"
	"sync"
	"time"

	"zerodyne/internal/monitor"
)

// ScanInterval is the background rescan period. NM scans take seconds;
// they run in the monitor goroutine, never on a client request path.
const ScanInterval = 30 * time.Second

// ScanMonitor is the registered "wifi" monitor.
type ScanMonitor struct {
	monitor.Base
	run    Runner
	mu     sync.Mutex
	dev    string
	nets   []ScanNetwork
	cancel context.CancelFunc
	done   chan struct{}
}

// NewScanMonitor builds the monitor with live system access.
func NewScanMonitor() *ScanMonitor {
	// Real NM scans take several seconds; the short default would kill
	// every background refresh.
	return &ScanMonitor{run: DefaultRunner{Timeout: 25 * time.Second}, done: make(chan struct{})}
}

// NewScanMonitorWithRunner builds the monitor with a stub runner (tests).
func NewScanMonitorWithRunner(r Runner) *ScanMonitor {
	return &ScanMonitor{run: r, done: make(chan struct{})}
}

// Name implements monitor.Monitor.
func (m *ScanMonitor) Name() string { return "wifi" }

// Snapshot implements monitor.Monitor: instant, never scans.
func (m *ScanMonitor) Snapshot(ctx context.Context) (any, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	nets := m.nets
	if nets == nil {
		nets = []ScanNetwork{}
	}
	return map[string]any{"device": m.dev, "networks": nets}, nil
}

// Start implements monitor.Monitor: warms the cache, then rescans in the
// background until Stop.
func (m *ScanMonitor) Start(ctx context.Context) error {
	ctx, cancel := context.WithCancel(ctx)
	m.cancel = cancel
	go m.loop(ctx)
	return nil
}

// Stop implements monitor.Monitor.
func (m *ScanMonitor) Stop() error {
	if m.cancel != nil {
		m.cancel()
	}
	<-m.done
	return nil
}

func (m *ScanMonitor) loop(ctx context.Context) {
	defer close(m.done)
	t := time.NewTicker(ScanInterval)
	defer t.Stop()
	// Refresh-then-wait (never wait-then-refresh): ticks coalesce behind
	// a slow scan instead of overlapping it.
	for {
		m.refresh(ctx)
		select {
		case <-ctx.Done():
			return
		case <-t.C:
		}
	}
}

// refresh runs one real scan into the cache. Slow path only.
func (m *ScanMonitor) refresh(ctx context.Context) {
	run := func(ctx context.Context, name string, args ...string) (string, error) {
		return m.run.Run(ctx, name, args...)
	}
	dev := ScanDevice(ctx, run)
	if dev == "" {
		return
	}
	nets, err := ScanVisible(ctx, run, dev, true)
	if err != nil {
		return
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	m.dev, m.nets = dev, nets
}
