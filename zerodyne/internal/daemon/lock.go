package daemon

import (
	"fmt"
	"os"
	"path/filepath"
	"syscall"
)

func dirOf(p string) string { return filepath.Dir(p) }

// pidLock is the held pidfile: the open fd keeps the flock alive.
type pidLock struct {
	f *os.File
}

func (l *pidLock) Close() error {
	path := l.f.Name()
	err := l.f.Close()
	// Only remove the pidfile if it still points at us; a newer daemon
	// would have replaced it after our lock was (unexpectedly) lost.
	if b, rerr := os.ReadFile(path); rerr == nil {
		var pid int
		if _, serr := fmt.Sscanf(string(b), "%d", &pid); serr == nil && pid == os.Getpid() {
			_ = os.Remove(path)
		}
	}
	return err
}

// lockPidFile takes an exclusive non-blocking flock on path and writes our
// pid. A second daemon gets a clear "already running" error instead of
// stealing the socket.
func lockPidFile(path string) (*pidLock, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, fmt.Errorf("daemon: pid dir: %w", err)
	}
	f, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o644)
	if err != nil {
		return nil, fmt.Errorf("daemon: open pidfile: %w", err)
	}
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		_ = f.Close()
		if pid, rerr := readPid(path); rerr == nil {
			return nil, fmt.Errorf("daemon already running (pid %d)", pid)
		}
		return nil, fmt.Errorf("daemon already running (pidfile locked)")
	}
	if err := f.Truncate(0); err != nil {
		_ = f.Close()
		return nil, fmt.Errorf("daemon: truncate pidfile: %w", err)
	}
	if _, err := fmt.Fprintf(f, "%d\n", os.Getpid()); err != nil {
		_ = f.Close()
		return nil, fmt.Errorf("daemon: write pidfile: %w", err)
	}
	return &pidLock{f: f}, nil
}

func readPid(path string) (int, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return 0, err
	}
	var pid int
	if _, err := fmt.Sscanf(string(b), "%d", &pid); err != nil {
		return 0, err
	}
	return pid, nil
}
