package network

import (
	"context"
	"strings"
	"testing"
	"time"
)

func TestBandForFreq(t *testing.T) {
	cases := map[string]string{
		"2412 MHz": "2.4", // nmcli form
		"2412":     "2.4",
		"5745.0":   "5", // iw form
		"5180 MHz": "5",
		"6135":     "6",
		"6":        "",
		"":         "",
		"abc":      "",
	}
	for in, want := range cases {
		got, ok := BandForFreq(in)
		if want == "" && ok {
			t.Errorf("BandForFreq(%q) = %q, want failure", in, got)
		}
		if want != "" && (!ok || got != want) {
			t.Errorf("BandForFreq(%q) = %q,%v want %q", in, got, ok, want)
		}
	}
}

func TestNmBandRoundTrip(t *testing.T) {
	for _, b := range []string{Band24, Band5, Band6} {
		nm, ok := NmBandFor(b)
		if !ok {
			t.Fatalf("NmBandFor(%q) failed", b)
		}
		if got := BandFromNm(nm); got != b {
			t.Fatalf("round trip %q -> %q -> %q", b, nm, got)
		}
	}
	if BandFromNm("whatever") != BandAuto {
		t.Fatal("unknown NM band should map to auto")
	}
}

func TestNormalizeBandTarget(t *testing.T) {
	if b, ok := NormalizeBandTarget("5220"); !ok || b != "5" {
		t.Fatalf("freq target: %q %v", b, ok)
	}
	if b, ok := NormalizeBandTarget("auto"); !ok || b != "auto" {
		t.Fatalf("auto target: %q %v", b, ok)
	}
	if _, ok := NormalizeBandTarget("bogus"); ok {
		t.Fatal("bogus target should fail")
	}
}

func TestThroughputTracker(t *testing.T) {
	tr := NewThroughputTracker()
	if d, u := tr.UpdateNanos("wlan0", 1000, 500, 1e9); d != 0 || u != 0 {
		t.Fatalf("first sample must reset, got %v %v", d, u)
	}
	d, u := tr.UpdateNanos("wlan0", 2000, 1000, 2e9)
	if d != 1000 || u != 500 {
		t.Fatalf("rates = %v %v, want 1000 500", d, u)
	}
	// Counter wrap must not go negative.
	if d, _ := tr.UpdateNanos("wlan0", 100, 100, 3e9); d != 0 {
		t.Fatalf("wrapped counter: %v", d)
	}
	// Interface change resets.
	if d, u := tr.UpdateNanos("eth0", 9999, 9999, 4e9); d != 0 || u != 0 {
		t.Fatalf("iface change must reset, got %v %v", d, u)
	}
}

// stubRunner answers canned outputs for the collector's command probes.
func stubRunner(files map[string]string) FuncRunner {
	return FuncRunner{Fn: func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		switch {
		case strings.HasPrefix(key, "ip -j route get"):
			return `[{"dev":"wlan0","gateway":"192.168.1.1","prefsrc":"192.168.1.5"}]`, nil
		case strings.HasPrefix(key, "ip -j addr show"):
			return `[{"addr_info":[{"family":"inet","prefixlen":24}]}]`, nil
		case strings.HasPrefix(key, "iw dev wlan0 link"):
			return "Connected to aa:bb:cc:dd:ee:ff\nSSID: Home Net\nfreq: 5745\nsignal: -55 dBm\ntx bitrate: 866.7 MBit/s", nil
		case strings.HasPrefix(key, "nmcli -t -f IN-USE,SIGNAL"):
			return "*:72\n :55", nil
		case strings.HasPrefix(key, "nmcli -t -f GENERAL.STATE"):
			return "GENERAL.STATE:100 (connected)\nGENERAL.CONNECTION:Home Net", nil
		case strings.HasPrefix(key, "ping"):
			return "64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=12.3 ms", nil
		}
		return "", errNoStub
	}}
}

type stubErr string

func (e stubErr) Error() string { return string(e) }

const errNoStub = stubErr("no stub")

func TestCollectorVerboseWifi(t *testing.T) {
	c := NewCollector(stubRunner(nil))
	c.IsWireless = func(dev string) bool { return dev == "wlan0" }
	c.Has = func(string) bool { return true }
	st, err := c.Collect(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if st.Iface != "wlan0" || st.Type != "wifi" {
		t.Fatalf("got %+v", st)
	}
	if st.SSID != "Home Net" || st.FreqMhz != 5745 {
		t.Fatalf("ssid/freq: %+v", st)
	}
	if st.SignalDbm == nil || *st.SignalDbm != -55 {
		t.Fatalf("signal dbm: %+v", st)
	}
	if st.SignalPct == nil || *st.SignalPct != 72 {
		t.Fatalf("signal pct: %+v", st)
	}
	if st.Bitrate != "866.7 MBit/s" {
		t.Fatalf("bitrate: %q", st.Bitrate)
	}
	if st.Prefix != 24 || st.Gateway != "192.168.1.1" || st.IP != "192.168.1.5" {
		t.Fatalf("route: %+v", st)
	}
	if st.InternetPingMs == nil || *st.InternetPingMs != 12.3 {
		t.Fatalf("ping: %+v", st)
	}
}

func TestCollectorNoRoute(t *testing.T) {
	c := NewCollector(FuncRunner{Fn: func(context.Context, string, ...string) (string, error) {
		return "", errNoStub
	}})
	c.Has = func(string) bool { return true }
	if _, err := c.Collect(context.Background()); err == nil {
		t.Fatal("expected no-route error")
	}
}

func TestCollectorMissingToolsDegrades(t *testing.T) {
	// No tools on PATH: the snapshot still resolves nothing but must not
	// fork anything. Route needs `ip`, so this is a no-route error, not a
	// hang or a fork failure.
	c := NewCollector(FuncRunner{Fn: func(context.Context, string, ...string) (string, error) {
		t.Error("no command should run when tools are missing")
		return "", errNoStub
	}})
	c.Has = func(string) bool { return false }
	if _, err := c.Collect(context.Background()); err == nil {
		t.Fatal("expected no-route error")
	}
}

func TestCollectorSSIDWithColon(t *testing.T) {
	// iw prints the SSID as the rest of the line; names containing ':'
	// must survive verbatim.
	c := NewCollector(FuncRunner{Fn: func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		switch {
		case strings.HasPrefix(key, "ip -j route"):
			return `[{"dev":"wlan0","gateway":"","prefsrc":"192.168.1.5"}]`, nil
		case strings.HasPrefix(key, "ip -j addr"):
			return `[{"addr_info":[{"family":"inet","prefixlen":24}]}]`, nil
		case strings.Contains(key, "GENERAL.STATE"):
			return "GENERAL.STATE:100 (connected)\nGENERAL.CONNECTION:--", nil
		case strings.Contains(key, "IN-USE"):
			return "", errNoStub
		case strings.HasPrefix(key, "iw "):
			return "SSID: odd:name\nfreq: 2412", nil
		}
		return "", errNoStub
	}})
	c.IsWireless = func(dev string) bool { return dev == "wlan0" }
	c.Has = func(string) bool { return true }
	st, err := c.Collect(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if st.SSID != "odd:name" || st.FreqMhz != 2412 {
		t.Fatalf("got %+v", st)
	}
}

func TestMonitorSnapshotAggregates(t *testing.T) {
	collector := NewCollector(stubRunner(nil))
	collector.IsWireless = func(dev string) bool { return dev == "wlan0" }
	collector.Has = func(string) bool { return true }
	m := &Monitor{collector: collector, tracker: NewThroughputTracker(), pings: NewPingTracker()}
	v, err := m.Snapshot(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	st, ok := v.(*Status)
	if !ok {
		t.Fatalf("type = %T", v)
	}
	// First snapshot: rates reset to zero, ping history holds one sample.
	if st.DownloadBps != 0 || st.UploadBps != 0 {
		t.Fatalf("rates = %v %v", st.DownloadBps, st.UploadBps)
	}
	if st.PingSamples != 1 {
		t.Fatalf("samples = %d", st.PingSamples)
	}
	if st.InternetPingAvgMs == nil || *st.InternetPingAvgMs != 12.3 {
		t.Fatalf("avg = %+v", st.InternetPingAvgMs)
	}
	if st.InternetLossPct != 0 {
		t.Fatalf("loss = %d", st.InternetLossPct)
	}
	if m.Name() != "network" {
		t.Fatalf("name = %q", m.Name())
	}
	_ = time.Now
}

func fptr(v float64) *float64 { return &v }

func TestPingTrackerAverageAndLoss(t *testing.T) {
	tr := NewPingTracker()
	// 4 successes then a timeout: average over last 5, loss 1/5.
	for _, v := range []float64{10, 20, 30, 40} {
		tr.Update("wlan0", fptr(v), fptr(v))
	}
	stats := tr.Update("wlan0", nil, nil)
	if stats.Samples != 5 {
		t.Fatalf("samples = %d", stats.Samples)
	}
	if stats.InternetAvg == nil || *stats.InternetAvg != 25 {
		t.Fatalf("avg = %+v", stats.InternetAvg)
	}
	if stats.InternetLossPct != 20 {
		t.Fatalf("loss = %d", stats.InternetLossPct)
	}
	// Window slides: after 24 more timeouts the history is all loss.
	for range PingHistoryWindow {
		stats = tr.Update("wlan0", nil, nil)
	}
	if stats.Samples != PingHistoryWindow {
		t.Fatalf("samples = %d", stats.Samples)
	}
	if stats.InternetAvg != nil {
		t.Fatalf("avg should be nil, got %v", *stats.InternetAvg)
	}
	if stats.InternetLossPct != 100 {
		t.Fatalf("loss = %d", stats.InternetLossPct)
	}
	// Interface change resets the history.
	stats = tr.Update("eth0", fptr(5), fptr(5))
	if stats.Samples != 1 || stats.InternetLossPct != 0 {
		t.Fatalf("reset = %+v", stats)
	}
}
