// Shared Wi-Fi helpers for the interaction handlers. Discovery and parsing
// live here; QML only renders what the handlers return.
package network

import (
	"context"
	"os"
	"os/exec"
	"strings"
	"time"
)

// liveRun executes system commands like wifiband's runner: combined output
// (nmcli reports failures on stderr) with LC_ALL=C pinned for nmcli.
func liveRun(ctx context.Context, name string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, 15*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, name, args...)
	if name == "nmcli" {
		cmd.Env = append(os.Environ(), "LC_ALL=C")
	}
	out, err := cmd.CombinedOutput()
	return strings.TrimRight(string(out), "\n"), err
}

// nmList runs `nmcli -e no -t -f fields ...`: -e no keeps ':'/'\' in values
// unescaped so SSIDs match iw's raw form.
func nmList(ctx context.Context, run runFunc, fields string, args ...string) (string, error) {
	full := append([]string{"-e", "no", "-t", "-f", fields}, args...)
	return run(ctx, "nmcli", full...)
}

// firstWifiDevice returns the connected Wi-Fi device, falling back to the
// first Wi-Fi device in any state. Empty when Wi-Fi is absent.
func firstWifiDevice(ctx context.Context, run runFunc) string {
	out, err := nmList(ctx, run, "DEVICE,TYPE,STATE", "device", "status")
	if err != nil {
		return ""
	}
	fallback := ""
	for _, line := range strings.Split(out, "\n") {
		parts := strings.SplitN(line, ":", 3)
		if len(parts) != 3 || parts[1] != "wifi" {
			continue
		}
		if fallback == "" {
			fallback = parts[0]
		}
		if strings.HasPrefix(parts[2], "connected") {
			return parts[0]
		}
	}
	return fallback
}

// routeDevice resolves the default-route device, mirroring
// `ip route get 1.1.1.1`.
func routeDevice(ctx context.Context, run runFunc) string {
	out, err := run(ctx, "ip", "route", "get", "1.1.1.1")
	if err != nil {
		return ""
	}
	fields := strings.Fields(out)
	for i, f := range fields {
		if f == "dev" && i+1 < len(fields) {
			return fields[i+1]
		}
	}
	return ""
}

// isWirelessDir reports whether dev has a wireless sysfs entry.
func isWirelessDir(dev string) bool {
	fi, err := os.Stat("/sys/class/net/" + dev + "/wireless")
	return err == nil && fi.IsDir()
}
