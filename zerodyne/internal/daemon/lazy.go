package daemon

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"syscall"
	"time"

	"zerodyne/internal/ipc"
	"zerodyne/internal/paths"
)

// Lazy client helpers. Every CLI request goes through EnsureRunning first:
//
//	zerodyne <request>
//	    ├── daemon answers → send over IPC
//	    └── daemon missing → spawn detached daemon → wait → send
//
// Spawning reuses the current executable (os.Executable) with "daemon".

// Alive reports whether a daemon answers at the default socket.
func Alive() bool { return ipc.Ping(paths.SockFile()) }

// EnsureRunning spawns a detached daemon if none answers. It returns true
// when it started one, false when one was already up.
func EnsureRunning(exe string) (bool, error) {
	if Alive() {
		if pid, err := readPid(paths.PidFile()); err == nil && !isRunning(pid) {
			// Socket answers but pidfile is stale/missing — treat as alive
			// anyway; the socket is the source of truth.
			_ = pid
		}
		return false, nil
	}
	// Stale pidfile without a socket: drop it so lockPidFile starts clean.
	if _, err := os.Stat(paths.SockFile()); os.IsNotExist(err) {
		_ = os.Remove(paths.PidFile())
	}
	if exe == "" {
		var err error
		exe, err = os.Executable()
		if err != nil {
			exe = os.Args[0]
		}
	}
	cmd := exec.Command(exe, "daemon")
	cmd.Stdin = nil
	cmd.Stdout = nil
	cmd.Stderr = nil
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return false, fmt.Errorf("start daemon: %w", err)
	}
	_ = cmd.Process.Release()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if Alive() {
			return true, nil
		}
		time.Sleep(100 * time.Millisecond)
	}
	return true, fmt.Errorf("daemon did not answer within 5s")
}

// Stop asks the daemon to exit gracefully, then waits for the socket to go
// quiet. It falls back to SIGTERM via the pidfile when IPC is dead.
func Stop() error {
	if Alive() {
		_, _ = ipc.Send(paths.SockFile(), ipc.Request{Op: ipc.OpStop})
		deadline := time.Now().Add(3 * time.Second)
		for time.Now().Before(deadline) {
			if !Alive() {
				return nil
			}
			time.Sleep(100 * time.Millisecond)
		}
	}
	pid, err := readPid(paths.PidFile())
	if err != nil {
		_ = os.Remove(paths.SockFile())
		_ = os.Remove(paths.PidFile())
		return fmt.Errorf("daemon not running")
	}
	if !isRunning(pid) {
		_ = os.Remove(paths.PidFile())
		_ = os.Remove(paths.SockFile())
		return fmt.Errorf("daemon not running (stale pid %d cleaned)", pid)
	}
	p, _ := os.FindProcess(pid)
	_ = p.Signal(syscall.SIGTERM)
	for range 30 {
		time.Sleep(100 * time.Millisecond)
		if !isRunning(pid) {
			return nil
		}
	}
	_ = p.Signal(syscall.SIGKILL)
	return nil
}

// Query ensures the daemon is up and sends req.
func Query(req ipc.Request) (ipc.Response, error) {
	if _, err := EnsureRunning(""); err != nil {
		return ipc.Response{}, err
	}
	// One retry: the daemon may have been mid-startup on the first dial.
	resp, err := ipc.Send(paths.SockFile(), req)
	if err != nil {
		time.Sleep(300 * time.Millisecond)
		resp, err = ipc.Send(paths.SockFile(), req)
	}
	return resp, err
}

func isRunning(pid int) bool {
	if pid <= 0 {
		return false
	}
	p, err := os.FindProcess(pid)
	if err != nil {
		return false
	}
	return p.Signal(syscall.Signal(0)) == nil
}

// DialDead reports whether the socket exists but answers nothing, for the
// CLI to distinguish "not running" from "wedged".
func DialDead() bool {
	_, err := net.DialTimeout("unix", paths.SockFile(), 200*time.Millisecond)
	return err != nil
}
