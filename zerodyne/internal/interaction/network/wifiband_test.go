package network

import (
	"context"
	"strings"
	"testing"
)

func stubRun(outputs map[string]string) runFunc {
	return func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		for prefix, out := range outputs {
			if strings.HasPrefix(key, prefix) {
				return out, nil
			}
		}
		return "", errStub("no stub for " + key)
	}
}

type errStub string

func (e errStub) Error() string { return string(e) }

func wifiUp() map[string]string {
	return map[string]string{
		"nmcli -e no -g DEVICE,TYPE,STATE":                         "wlan0:wifi:connected\neth0:ethernet:disconnected",
		"nmcli -e no -g GENERAL.CONNECTION device show wlan0":      "Home",
		"iw dev wlan0 link":                                        "Connected to aa:bb\nSSID: Home\nfreq: 5745\nsignal: -55 dBm",
		"nmcli -e no -g FREQ,SSID":                                 "2412:Other\n5745:Home\n6135:Home",
		"nmcli -e no -g 802-11-wireless.band connection show Home": "a",
	}
}

func TestQueryStatus(t *testing.T) {
	h := NewHandlerWithRunner(stubRun(wifiUp()))
	res, err := h.Exec(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	if res.Data["band"] != "5" {
		t.Fatalf("band = %v", res.Data)
	}
	if res.Data["selected"] != "5" {
		t.Fatalf("selected = %v", res.Data)
	}
	avail, _ := res.Data["available"].([]string)
	if len(avail) != 2 || avail[0] != "5" || avail[1] != "6" {
		t.Fatalf("available = %v", res.Data["available"])
	}
}

func TestQueryNoDevice(t *testing.T) {
	h := NewHandlerWithRunner(stubRun(map[string]string{
		"nmcli -e no -g DEVICE,TYPE,STATE": "eth0:ethernet:connected",
	}))
	if _, err := h.Exec(context.Background(), nil); err == nil {
		t.Fatal("expected no-device error")
	}
}

func TestSetBandValidation(t *testing.T) {
	h := NewHandlerWithRunner(stubRun(wifiUp()))
	if _, err := h.Exec(context.Background(), []string{"bogus"}); err == nil {
		t.Fatal("expected invalid-band error")
	}
	if _, err := h.Exec(context.Background(), []string{"5", "extra"}); err == nil {
		t.Fatal("expected arity error")
	}
	// 2.4 GHz is not in this network's scan results → refused like the script.
	if _, err := h.Exec(context.Background(), []string{"2.4"}); err == nil {
		t.Fatal("expected unavailable-band error")
	}
}

func TestSetBandRevertsOnFailure(t *testing.T) {
	var modified []string
	base := wifiUp()
	run := func(ctx context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		if strings.HasPrefix(key, "nmcli connection up") {
			return "", errStub("association failed")
		}
		if strings.HasPrefix(key, "nmcli connection modify") {
			modified = append(modified, key)
			return "", nil
		}
		return stubRun(base)(ctx, name, args...)
	}
	h := NewHandlerWithRunner(run)
	if _, err := h.Exec(context.Background(), []string{"6"}); err == nil {
		t.Fatal("expected reassociation error")
	}
	if len(modified) != 2 {
		t.Fatalf("expected set+revert modifies, got %v", modified)
	}
	if !strings.HasSuffix(modified[1], " a") {
		t.Fatalf("revert should restore previous band: %v", modified)
	}
}

func TestNameUsage(t *testing.T) {
	h := NewHandler()
	if h.Name() != "wifi-band" || h.Usage() == "" {
		t.Fatalf("name/usage = %q %q", h.Name(), h.Usage())
	}
}
