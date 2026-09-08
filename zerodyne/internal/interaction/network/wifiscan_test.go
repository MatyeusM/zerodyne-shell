package network

import (
	"context"
	"strings"
	"testing"

	netmon "zerodyne/internal/monitor/network"
)

func stubScan(outputs map[string]string) runFunc {
	return func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		for prefix, out := range outputs {
			if strings.HasPrefix(key, prefix) {
				return out, nil
			}
		}
		return "", nil
	}
}

func TestScanDedupesToStrongestAP(t *testing.T) {
	run := stubScan(map[string]string{
		"nmcli -e no -t -f DEVICE,TYPE,STATE device status": "wlan0:wifi:connected\n",
		"nmcli -e no -t -f IN-USE":                          "*:80:WPA2:AA:BB:CC:DD:EE:FF:Home\n:70:WPA2:BB:CC:DD:EE:FF:00:Home\n:60::CC:DD:EE:FF:00:11:Open\n",
		"nmcli -e no -t -f NAME connection show":            "Home\n",
	})
	res, err := NewScanHandlerWithRunner(run).Exec(context.Background(), nil)
	if err != nil {
		t.Fatalf("exec: %v", err)
	}
	nets, ok := res.Data["networks"].([]netmon.ScanNetwork)
	if !ok || len(nets) != 2 {
		t.Fatalf("want 2 SSID rows, got %#v", res.Data["networks"])
	}
	if nets[0].SSID != "Home" || !nets[0].InUse || !nets[0].Known || nets[0].Signal != 80 {
		t.Fatalf("first row wrong: %+v", nets[0])
	}
	if nets[1].SSID != "Open" || nets[1].Security != "open" {
		t.Fatalf("second row wrong: %+v", nets[1])
	}
}

func TestScanSkipsEmptySSIDAndBadSignal(t *testing.T) {
	run := stubScan(map[string]string{
		"nmcli -e no -t -f DEVICE,TYPE,STATE device status": "wlan0:wifi:disconnected\n",
		"nmcli -e no -t -f IN-USE":                          ":50:WPA2:AA:\nCorp:xx:WPA2:BB:Corp\n",
	})
	res, err := NewScanHandlerWithRunner(run).Exec(context.Background(), nil)
	if err != nil {
		t.Fatalf("exec: %v", err)
	}
	nets := res.Data["networks"].([]netmon.ScanNetwork)
	if len(nets) != 0 {
		t.Fatalf("want no networks, got %+v", nets)
	}
}

func TestScanRejectsBadArgs(t *testing.T) {
	run := stubScan(nil)
	if _, err := NewScanHandlerWithRunner(run).Exec(context.Background(), []string{"a", "b"}); err == nil {
		t.Fatal("want usage error")
	}
}
