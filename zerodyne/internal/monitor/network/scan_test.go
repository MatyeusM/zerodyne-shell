package network

import (
	"context"
	"strings"
	"testing"
)

func stubMonitor(outputs map[string]string) Runner {
	return FuncRunner{Fn: func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		for prefix, out := range outputs {
			if strings.HasPrefix(key, prefix) {
				return out, nil
			}
		}
		return "", nil
	}}
}

func TestScanVisibleKeepsColonSSIDs(t *testing.T) {
	run := stubMonitor(map[string]string{
		"nmcli -e no -t -f NAME connection show": "My:Home\n",
		"nmcli -e no -t -f IN-USE":               "*:80:WPA2:AA:BB:CC:DD:EE:FF:My:Home\n:70:WPA2:BB:CC:DD:EE:FF:00:My:Home\n",
	})
	nets, err := ScanVisible(context.Background(), adaptTest(run), "wlan0", false)
	if err != nil {
		t.Fatalf("scan: %v", err)
	}
	if len(nets) != 1 || nets[0].SSID != "My:Home" {
		t.Fatalf("colon SSIDs wrong: %+v", nets)
	}
	if nets[0].BSSID != "AA:BB:CC:DD:EE:FF" || !nets[0].InUse || !nets[0].Known {
		t.Fatalf("first row wrong: %+v", nets[0])
	}
}

func TestScanMonitorServesCache(t *testing.T) {
	run := stubMonitor(map[string]string{
		"nmcli -e no -t -f DEVICE,TYPE,STATE device status": "wlan0:wifi:connected\n",
		"nmcli -e no -t -f NAME connection show":            "Home\n",
		"nmcli -e no -t -f IN-USE":                          "*:80:WPA2:AA:BB:CC:DD:EE:FF:Home\n",
	})
	m := NewScanMonitorWithRunner(run)
	m.refresh(context.Background())
	data, err := m.Snapshot(context.Background())
	if err != nil {
		t.Fatalf("snapshot: %v", err)
	}
	payload, ok := data.(map[string]any)
	if !ok {
		t.Fatalf("payload shape: %#v", data)
	}
	nets, ok := payload["networks"].([]ScanNetwork)
	if !ok || len(nets) != 1 || payload["device"] != "wlan0" {
		t.Fatalf("cache wrong: %#v", payload)
	}
}

func adaptTest(r Runner) RunFunc {
	return func(ctx context.Context, name string, args ...string) (string, error) {
		return r.Run(ctx, name, args...)
	}
}
