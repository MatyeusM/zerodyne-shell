// Package monitor defines the Monitor abstraction and its Registry.
//
// A Monitor is read-only observation of system state: it answers "what is
// true right now" and never changes anything (see the interaction package
// for state-changing operations).
//
// Snapshot data (SSID, CPU usage, ...) is queried on demand. Accumulated
// data (throughput, moving averages, ...) is maintained by monitors that
// opt into background collection via Start; the daemon starts/stops them
// with its own lifecycle and keeps them alive while clients are interested.
package monitor

import (
	"context"
	"fmt"
	"sort"
	"sync"
)

// Monitor exposes one named information domain (network, performance, ...).
type Monitor interface {
	// Name is the `status <target>` selector, e.g. "network".
	Name() string
	// Snapshot returns the current state. It must be safe for concurrent
	// use and respect ctx cancellation/timeout.
	Snapshot(ctx context.Context) (any, error)
	// Start begins optional background collection; the default is a no-op.
	Start(ctx context.Context) error
	// Stop ends background collection; the default is a no-op.
	Stop() error
}

// Base provides no-op Start/Stop for monitors that are purely on-demand.
type Base struct{}

func (Base) Start(context.Context) error { return nil }
func (Base) Stop() error                 { return nil }

// Registry maps monitor names to monitors. Register once at daemon startup;
// Snapshot dispatches `status <target>` without touching other monitors.
type Registry struct {
	mu       sync.RWMutex
	monitors map[string]Monitor
}

// NewRegistry builds an empty registry.
func NewRegistry() *Registry {
	return &Registry{monitors: map[string]Monitor{}}
}

// Register adds a monitor. Registering the same name twice is an error so
// accidental shadowing fails loudly at startup, not silently at runtime.
func (r *Registry) Register(m Monitor) error {
	if m == nil || m.Name() == "" {
		return fmt.Errorf("monitor: nil or unnamed monitor")
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, dup := r.monitors[m.Name()]; dup {
		return fmt.Errorf("monitor: duplicate %q", m.Name())
	}
	r.monitors[m.Name()] = m
	return nil
}

// MustRegister is Register for startup wiring where failure is fatal.
func (r *Registry) MustRegister(m Monitor) {
	if err := r.Register(m); err != nil {
		panic(err)
	}
}

// Snapshot dispatches to the named monitor.
func (r *Registry) Snapshot(ctx context.Context, name string) (any, error) {
	r.mu.RLock()
	m, ok := r.monitors[name]
	r.mu.RUnlock()
	if !ok {
		return nil, fmt.Errorf("unknown status target %q (known: %s)", name, r.Names())
	}
	return m.Snapshot(ctx)
}

// StartAll starts every monitor that wants background collection.
func (r *Registry) StartAll(ctx context.Context) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	for _, m := range r.monitors {
		_ = m.Start(ctx)
	}
}

// StopAll stops every monitor. Errors are ignored: shutdown is best-effort.
func (r *Registry) StopAll() {
	r.mu.RLock()
	defer r.mu.RUnlock()
	for _, m := range r.monitors {
		_ = m.Stop()
	}
}

// Names returns the sorted registered monitor names (for errors/help).
func (r *Registry) Names() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()
	names := make([]string, 0, len(r.monitors))
	for n := range r.monitors {
		names = append(names, n)
	}
	sort.Strings(names)
	return names
}
