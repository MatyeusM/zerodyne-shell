// Package paths resolves zerodyne's runtime locations.
//
// The daemon communicates over a Unix socket. Prefer XDG_RUNTIME_DIR
// (per-user, tmpfs, cleaned on logout); fall back to XDG_STATE_HOME
// (~/.local/state) so headless/CI runs still work.
package paths

import (
	"os"
	"path/filepath"
)

// Dir returns the directory holding the socket and pidfile.
func Dir() string {
	if v := os.Getenv("XDG_RUNTIME_DIR"); v != "" {
		return filepath.Join(v, "zerodyne")
	}
	return filepath.Join(StateDir(), "zerodyne")
}

// StateDir returns the persistent state directory.
func StateDir() string {
	if v := os.Getenv("XDG_STATE_HOME"); v != "" {
		return filepath.Join(v, "zerodyne")
	}
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".local", "state", "zerodyne")
}

// SockFile returns the Unix socket path.
func SockFile() string { return filepath.Join(Dir(), "zerodyne.sock") }

// PidFile returns the pidfile path.
func PidFile() string { return filepath.Join(Dir(), "zerodyne.pid") }
