// Visible-network collection shared by the scan monitor (background
// cache) and the wifi-scan interaction (forced refresh). One row per
// access point: same-SSID APs stay distinct by BSSID so every network
// the radio sees is listed.
package network

import (
	"context"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// ScanNetwork is one visible access point.
type ScanNetwork struct {
	SSID     string `json:"ssid"`
	BSSID    string `json:"bssid"`
	Signal   int    `json:"signal"`   // 0-100
	Security string `json:"security"` // open|wep|wpa|eap
	InUse    bool   `json:"in_use"`
	Known    bool   `json:"known"` // a saved profile exists for the SSID
}

// RunFunc executes a system command. It matches both monitor.Runner.Run
// and the interaction runFunc shape so both layers share this code.
type RunFunc func(ctx context.Context, name string, args ...string) (string, error)

// ScanDevice returns the connected Wi-Fi device, falling back to the
// first Wi-Fi device in any state. Empty when Wi-Fi is absent.
func ScanDevice(ctx context.Context, run RunFunc) string {
	out, err := run(ctx, "nmcli", "-e", "no", "-t", "-f", "DEVICE,TYPE,STATE", "device", "status")
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

// ScanKnown returns the set of saved connection names.
func ScanKnown(ctx context.Context, run RunFunc) map[string]bool {
	set := map[string]bool{}
	out, err := run(ctx, "nmcli", "-e", "no", "-t", "-f", "NAME", "connection", "show")
	if err != nil {
		return set
	}
	for _, line := range strings.Split(out, "\n") {
		if name := strings.TrimSpace(line); name != "" {
			set[name] = true
		}
	}
	return set
}

// bssidRe splits the BSSID off the SSID. The BSSID itself contains
// colons, so the line is split into its first three colon-free fields
// and the remainder is matched as MAC + rest-is-SSID.
var bssidRe = regexp.MustCompile(`^((?:[0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}):(.*)$`)

// ScanVisible lists visible access points. rescan forces a real NM scan
// (slow); otherwise the NM cache is read. SSID goes last so names
// containing ':' survive verbatim.
func ScanVisible(ctx context.Context, run RunFunc, dev string, rescan bool) ([]ScanNetwork, error) {
	mode := "no"
	if rescan {
		mode = "yes"
	}
	out, err := run(ctx, "nmcli", "-e", "no", "-t", "-f", "IN-USE,SIGNAL,SECURITY,BSSID,SSID",
		"dev", "wifi", "list", "ifname", dev, "--rescan", mode)
	if err != nil {
		return nil, err
	}
	known := ScanKnown(ctx, run)
	best := map[string]ScanNetwork{}
	for _, line := range strings.Split(out, "\n") {
		parts := strings.SplitN(line, ":", 4)
		if len(parts) != 4 {
			continue
		}
		sig, err := strconv.Atoi(strings.TrimSpace(parts[1]))
		if err != nil {
			continue
		}
		mac := bssidRe.FindStringSubmatch(parts[3])
		if mac == nil {
			continue
		}
		ssid := strings.TrimSpace(mac[2])
		if ssid == "" {
			continue
		}
		row := ScanNetwork{
			SSID:     ssid,
			BSSID:    mac[1],
			Signal:   sig,
			Security: wifiSecClass(parts[2]),
			InUse:    strings.TrimSpace(parts[0]) == "*",
			Known:    known[ssid],
		}
		// One row per SSID: keep the strongest AP, sticky in-use flag.
		if cur, ok := best[ssid]; !ok || row.Signal > cur.Signal {
			row.InUse = row.InUse || cur.InUse
			best[ssid] = row
		} else if row.InUse {
			cur.InUse = true
			best[ssid] = cur
		}
	}
	var nets []ScanNetwork
	for _, n := range best {
		nets = append(nets, n)
	}
	sort.Slice(nets, func(i, j int) bool {
		if nets[i].InUse != nets[j].InUse {
			return nets[i].InUse
		}
		if nets[i].Signal != nets[j].Signal {
			return nets[i].Signal > nets[j].Signal
		}
		if nets[i].SSID != nets[j].SSID {
			return nets[i].SSID < nets[j].SSID
		}
		return nets[i].BSSID < nets[j].BSSID
	})
	return nets, nil
}

// wifiSecClass reduces NM's security string to a UI class.
func wifiSecClass(s string) string {
	u := strings.ToUpper(s)
	if strings.Contains(u, "802.1X") || strings.Contains(u, "EAP") {
		return "eap"
	}
	if strings.Contains(u, "WEP") {
		return "wep"
	}
	if strings.Contains(u, "WPA") || strings.Contains(u, "SAE") || strings.Contains(u, "OWE") {
		return "wpa"
	}
	return "open"
}
