// Package caps probes which system tools are available, replacing the
// shell scripts' cmd-present.sh checks (and the stale cmd-present
// reference in network-speedtest.sh).
//
// Probing happens once per process via LookPath and is cached: monitors
// skip probes whose tool is missing instead of forking and failing, and
// QML can read the verdict from daemon info to render "unavailable" states
// instead of polling a dead backend.
package caps

import (
	"os/exec"
	"sync"
)

// Tools is the set of binaries zerodyne may shell out to. Keep it to tools
// the migration actually needs; add entries when a monitor gains a probe.
var Tools = []string{"ip", "nmcli", "iw", "ping", "qrencode", "curl", "pactl", "wpctl", "systemctl", "hyprctl", "hyprlock", "amd-smi", "powerprofilesctl"}

var mu sync.RWMutex

var cached map[string]bool

// Probe checks every known tool once and caches the result.
func Probe() map[string]bool {
	mu.RLock()
	if cached != nil {
		defer mu.RUnlock()
		return cached
	}
	mu.RUnlock()

	mu.Lock()
	defer mu.Unlock()
	if cached == nil {
		cached = probe(Tools)
	}
	return cached
}

// Available reports whether name is on PATH, consulting the cache.
func Available(name string) bool {
	mu.RLock()
	if cached != nil {
		v, ok := cached[name]
		mu.RUnlock()
		if ok {
			return v
		}
		// Unknown tool: fall through to a direct check.
	} else {
		mu.RUnlock()
	}
	ok := lookPath(name)
	mu.Lock()
	if cached == nil {
		cached = map[string]bool{}
	}
	cached[name] = ok
	mu.Unlock()
	return ok
}

// Refresh re-probes (PATH may have changed; tests use this too).
func Refresh() map[string]bool {
	mu.Lock()
	defer mu.Unlock()
	cached = probe(Tools)
	return cached
}

func probe(names []string) map[string]bool {
	out := make(map[string]bool, len(names))
	for _, n := range names {
		out[n] = lookPath(n)
	}
	return out
}

func lookPath(name string) bool {
	_, err := exec.LookPath(name)
	return err == nil
}
