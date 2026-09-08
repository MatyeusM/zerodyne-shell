// Package interaction defines the Handler abstraction and its Registry.
//
// Handlers change system state (wifi-band, wifi-connect, ...) and are
// deliberately separate from monitors (which only observe). Each handler:
//
//   - has a clear name used as the `exec <operation>` selector,
//   - validates its own arguments and reports usage on misuse,
//   - returns a structured result or a typed error,
//   - is added via Registry.Register with no central switch to edit.
package interaction

import (
	"context"
	"fmt"
	"sort"
	"sync"
)

// Result is the structured outcome of a handler execution.
type Result struct {
	Message string         `json:"message,omitempty"`
	Data    map[string]any `json:"data,omitempty"`
}

// Handler performs one state-changing operation.
type Handler interface {
	// Name is the `exec <operation>` selector, e.g. "wifi-band".
	Name() string
	// Usage is a one-line synopsis shown on argument errors.
	Usage() string
	// Exec validates args, performs the operation, and returns the result.
	Exec(ctx context.Context, args []string) (Result, error)
}

// Registry maps operation names to handlers.
type Registry struct {
	mu       sync.RWMutex
	handlers map[string]Handler
}

// NewRegistry builds an empty registry.
func NewRegistry() *Registry {
	return &Registry{handlers: map[string]Handler{}}
}

// Register adds a handler. Duplicate names are an error.
func (r *Registry) Register(h Handler) error {
	if h == nil || h.Name() == "" {
		return fmt.Errorf("interaction: nil or unnamed handler")
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, dup := r.handlers[h.Name()]; dup {
		return fmt.Errorf("interaction: duplicate %q", h.Name())
	}
	r.handlers[h.Name()] = h
	return nil
}

// MustRegister is Register for startup wiring where failure is fatal.
func (r *Registry) MustRegister(h Handler) {
	if err := r.Register(h); err != nil {
		panic(err)
	}
}

// Exec dispatches to the named handler.
func (r *Registry) Exec(ctx context.Context, name string, args []string) (Result, error) {
	r.mu.RLock()
	h, ok := r.handlers[name]
	r.mu.RUnlock()
	if !ok {
		return Result{}, fmt.Errorf("unknown operation %q (known: %s)", name, r.Names())
	}
	return h.Exec(ctx, args)
}

// Names returns the sorted registered handler names.
func (r *Registry) Names() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()
	names := make([]string, 0, len(r.handlers))
	for n := range r.handlers {
		names = append(names, n)
	}
	sort.Strings(names)
	return names
}
