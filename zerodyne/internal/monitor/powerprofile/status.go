package powerprofile

import (
	"context"
	"fmt"
	"strings"

	"zerodyne/internal/caps"
)

// KnownProfiles is the daemon's fixed profile set. The list output is
// validated against it so indented detail lines never leak in.
var KnownProfiles = []string{"performance", "balanced", "power-saver"}

// Status is the power-profile state.
type Status struct {
	Active   string   `json:"active"`
	Profiles []string `json:"profiles"`
}

// Collector gathers power-profile state through a Runner.
type Collector struct {
	run Runner
	// Has reports whether a tool is available. Defaults to the cached
	// caps probe; tests stub it.
	Has func(tool string) bool
}

// NewCollector builds a Collector. A nil runner means live system access.
func NewCollector(r Runner) *Collector {
	if r == nil {
		r = DefaultRunner{}
	}
	return &Collector{run: r, Has: caps.Available}
}

// Collect snapshots the active profile and the available set.
func (c *Collector) Collect(ctx context.Context) (*Status, error) {
	if !c.Has("powerprofilesctl") {
		return nil, fmt.Errorf("power-profile: powerprofilesctl not available")
	}
	out, err := c.run.Run(ctx, "powerprofilesctl", "list")
	if err != nil {
		return nil, fmt.Errorf("power-profile: list: %w", err)
	}
	active, profiles := ParseList(out)
	if out, err := c.run.Run(ctx, "powerprofilesctl", "get"); err == nil {
		if got := strings.TrimSpace(out); got != "" {
			active = got
		}
	}
	return &Status{Active: active, Profiles: profiles}, nil
}

// ParseList decodes `powerprofilesctl list`: top-level "<name>:" lines in
// list order, the active one prefixed with "* ". Pure, for tests.
func ParseList(out string) (active string, profiles []string) {
	known := map[string]bool{}
	for _, p := range KnownProfiles {
		known[p] = true
	}
	profiles = []string{}
	for _, line := range strings.Split(out, "\n") {
		// Profiles sit at 0-2 spaces indent; their detail lines at 4+.
		indent := len(line) - len(strings.TrimLeft(line, " \t"))
		if indent > 2 {
			continue
		}
		trimmed := strings.TrimSpace(line)
		if trimmed == "" {
			continue
		}
		name := strings.TrimSuffix(trimmed, ":")
		name, isActive := strings.CutPrefix(name, "* ")
		name = strings.TrimSpace(name)
		if !known[name] {
			continue
		}
		profiles = append(profiles, name)
		if isActive {
			active = name
		}
	}
	return active, profiles
}

// ValidProfile reports whether name is settable.
func ValidProfile(name string) bool {
	for _, p := range KnownProfiles {
		if p == name {
			return true
		}
	}
	return false
}
