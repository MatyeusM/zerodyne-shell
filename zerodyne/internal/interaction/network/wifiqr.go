package network

import (
	"context"
	"fmt"
	"strings"

	"zerodyne/internal/caps"
	"zerodyne/internal/interaction"
)

// QrHandler implements "wifi-qr": the share card payload for the active
// Wi-Fi connection. It ports services/network-qr.sh: secret lookup,
// payload encoding, and qrencode matrix collapsing all happen here; QML
// renders the returned rows as rectangles.
type QrHandler struct {
	run runFunc
}

// NewQrHandler builds the handler with live system access.
func NewQrHandler() *QrHandler { return &QrHandler{run: liveRun} }

// NewQrHandlerWithRunner builds the handler with a stub runner (tests).
func NewQrHandlerWithRunner(run runFunc) *QrHandler { return &QrHandler{run: run} }

// Name implements interaction.Handler.
func (h *QrHandler) Name() string { return "wifi-qr" }

// Usage implements interaction.Handler.
func (h *QrHandler) Usage() string { return "wifi-qr" }

// QrData is the share card: connection facts plus the collapsed matrix.
type QrData struct {
	Iface    string   `json:"iface"`
	SSID     string   `json:"ssid"`
	Security string   `json:"security"` // WPA|WEP|nopass
	Password string   `json:"password,omitempty"`
	Size     int      `json:"size"`
	Rows     []string `json:"rows"` // "0"/"1" per module, row-major
}

// Exec implements interaction.Handler.
func (h *QrHandler) Exec(ctx context.Context, args []string) (interaction.Result, error) {
	if len(args) != 0 {
		return interaction.Result{}, fmt.Errorf("usage: %s", h.Usage())
	}
	if !caps.Available("qrencode") {
		return interaction.Result{}, fmt.Errorf("wifi-qr: qrencode is not installed")
	}
	dev := routeDevice(ctx, h.run)
	if dev == "" || !isWirelessDir(dev) {
		dev = firstWifiDevice(ctx, h.run)
	}
	if dev == "" {
		return interaction.Result{}, fmt.Errorf("wifi-qr: no active wifi connection")
	}
	out, err := h.run(ctx, "nmcli", "--get-values", "GENERAL.CON-UUID", "device", "show", dev)
	uuid := strings.TrimSpace(out)
	if err != nil || uuid == "" || uuid == "--" {
		return interaction.Result{}, fmt.Errorf("wifi-qr: no active wifi connection")
	}
	fields, err := h.secrets(ctx, uuid)
	if err != nil {
		return interaction.Result{}, err
	}
	data, err := buildQr(ctx, h.run, dev, fields)
	if err != nil {
		return interaction.Result{}, err
	}
	return interaction.Result{Data: map[string]any{
		"iface":    data.Iface,
		"ssid":     data.SSID,
		"security": data.Security,
		"password": data.Password,
		"size":     data.Size,
		"rows":     data.Rows,
	}}, nil
}

type qrFields struct {
	ssid     string
	keyMgmt  string
	password string
	hidden   string
	wepKey   string
}

func (h *QrHandler) secrets(ctx context.Context, uuid string) (qrFields, error) {
	out, err := h.run(ctx, "nmcli", "--show-secrets", "--escape", "no", "--get-values",
		"802-11-wireless.ssid,802-11-wireless-security.key-mgmt,802-11-wireless-security.psk,802-11-wireless.hidden,802-11-wireless-security.wep-key0",
		"connection", "show", "uuid", uuid)
	if err != nil {
		return qrFields{}, fmt.Errorf("wifi-qr: cannot read connection secrets")
	}
	lines := strings.Split(out, "\n")
	var f qrFields
	if len(lines) > 0 {
		f.ssid = strings.TrimSpace(lines[0])
	}
	if len(lines) > 1 {
		f.keyMgmt = strings.TrimSpace(lines[1])
	}
	if len(lines) > 2 {
		f.password = strings.TrimSpace(lines[2])
	}
	if len(lines) > 3 {
		f.hidden = strings.TrimSpace(lines[3])
	}
	if len(lines) > 4 {
		f.wepKey = strings.TrimSpace(lines[4])
	}
	return f, nil
}

// buildQr classifies security, encodes the WIFI: payload, and collapses
// qrencode's two-characters-per-module ASCII into 0/1 rows.
func buildQr(ctx context.Context, run runFunc, dev string, f qrFields) (QrData, error) {
	if f.ssid == "" {
		return QrData{}, fmt.Errorf("wifi-qr: cannot read the wifi name")
	}
	if strings.Contains(f.keyMgmt, "eap") || strings.Contains(f.keyMgmt, "ieee8021x") {
		return QrData{}, fmt.Errorf("wifi-qr: enterprise wifi cannot be shared with a password qr code")
	}
	data := QrData{Iface: dev, SSID: f.ssid}
	switch {
	case f.keyMgmt != "" && f.keyMgmt != "none":
		if f.password == "" {
			return QrData{}, fmt.Errorf("wifi-qr: cannot read the wifi password")
		}
		data.Security, data.Password = "WPA", f.password
	case f.wepKey != "":
		data.Security, data.Password = "WEP", f.wepKey
	default:
		data.Security = "nopass"
	}
	payload := "WIFI:T:" + data.Security + ";S:" + escapeQrValue(f.ssid) + ";P:" + escapeQrValue(data.Password) + ";"
	if f.hidden == "yes" {
		payload += "H:true;"
	}
	payload += ";"
	ascii, err := run(ctx, "qrencode", "--type", "ASCII", "--margin", "4", "--output", "-", payload)
	if err != nil {
		return QrData{}, fmt.Errorf("wifi-qr: qrencode failed")
	}
	data.Rows = collapseQrMatrix(ascii)
	if len(data.Rows) == 0 {
		return QrData{}, fmt.Errorf("wifi-qr: qrencode produced no matrix")
	}
	data.Size = len(data.Rows)
	return data, nil
}

// escapeQrValue escapes the WIFI: value separators.
func escapeQrValue(v string) string {
	v = strings.ReplaceAll(v, `\`, `\\`)
	v = strings.ReplaceAll(v, ";", `\;`)
	v = strings.ReplaceAll(v, ",", `\,`)
	v = strings.ReplaceAll(v, ":", `\:`)
	return v
}

// collapseQrMatrix turns two-characters-per-module ASCII rows into 0/1
// strings: a pair counts as set when it contains '#'.
func collapseQrMatrix(ascii string) []string {
	var rows []string
	for _, line := range strings.Split(ascii, "\n") {
		if strings.TrimSpace(line) == "" {
			continue
		}
		var row strings.Builder
		for i := 0; i < len(line); i += 2 {
			pair := line[i:min(i+2, len(line))]
			if strings.Contains(pair, "#") {
				row.WriteByte('1')
			} else {
				row.WriteByte('0')
			}
		}
		rows = append(rows, row.String())
	}
	return rows
}
