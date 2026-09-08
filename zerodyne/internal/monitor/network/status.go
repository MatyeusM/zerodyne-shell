package network

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"zerodyne/internal/caps"
)

// InternetProbe mirrors network-status.sh's internet_probe: the address the
// default route is resolved against.
const InternetProbe = "1.1.1.1"

// Status is the single network status shape: the typed form of
// `network-status.sh --verbose`, plus daemon-side aggregation. It is always
// served as JSON — QML parses it directly instead of splitting tab lines.
//
// Optional fields use pointers so "unknown" and "zero" stay distinguishable
// (previously: empty QML strings rendered as "--").
type Status struct {
	Iface   string `json:"iface"`
	IP      string `json:"ip,omitempty"`
	Prefix  int    `json:"prefix,omitempty"`
	Gateway string `json:"gateway,omitempty"`
	Type    string `json:"type"` // wifi | ethernet
	RxBytes uint64 `json:"rx_bytes"`
	TxBytes uint64 `json:"tx_bytes"`
	SSID    string `json:"ssid,omitempty"`

	SignalPct *int   `json:"signal_pct,omitempty"` // nmcli 0-100, wifi only
	SignalDbm *int   `json:"signal_dbm,omitempty"` // iw dBm, wifi only
	FreqMhz   int    `json:"freq_mhz,omitempty"`
	Bitrate   string `json:"bitrate,omitempty"`  // e.g. "866.7 MBit/s"
	SpeedMb   *int   `json:"speed_mb,omitempty"` // sysfs, ethernet only
	Duplex    string `json:"duplex,omitempty"`   // sysfs, ethernet only

	// Latest poll's ping samples; nil when unprobed or timed out.
	RouterPingMs   *float64 `json:"router_ping_ms"`
	InternetPingMs *float64 `json:"internet_ping_ms"`

	// Accumulated data derived from consecutive snapshots (previously
	// NetworkModel.js throughputState/pingLatencyState in QML).
	DownloadBps float64 `json:"download_bps"`
	UploadBps   float64 `json:"upload_bps"`

	// Ping history aggregates over PingHistoryWindow polls (nil when no
	// successful sample yet); loss counts timeouts like QML's null entries.
	RouterPingAvgMs   *float64 `json:"router_ping_avg_ms"`
	InternetPingAvgMs *float64 `json:"internet_ping_avg_ms"`
	InternetLossPct   int      `json:"internet_loss_pct"`
	PingSamples       int      `json:"ping_samples"`
}

// Collector gathers network state through a Runner.
type Collector struct {
	run Runner
	// IsWireless reports whether dev is a wireless interface. Defaults to
	// the /sys/class/net/<dev>/wireless probe; tests stub it.
	IsWireless func(dev string) bool
	// Has reports whether a tool is available. Defaults to the cached
	// caps probe; tests stub it. Missing tools are skipped instead of
	// forked-and-failed, so minimal systems get fast degraded snapshots.
	Has func(tool string) bool
}

// NewCollector builds a Collector. A nil runner means live system access.
func NewCollector(r Runner) *Collector {
	if r == nil {
		r = DefaultRunner{}
	}
	return &Collector{run: r, IsWireless: isWireless, Has: caps.Available}
}

// Collect is the one network status: a port of print_verbose from
// network-status.sh. The old non-verbose print_status is gone — its two
// derived values (kind label, header frequency) are trivially derived from
// Type/SSID/Iface/FreqMhz on the consumer side.
func (c *Collector) Collect(ctx context.Context) (*Status, error) {
	route := c.routeGet(ctx)
	if route.dev == "" {
		return nil, fmt.Errorf("network: no default route")
	}
	st := &Status{Iface: route.dev, IP: route.src, Gateway: route.gw, Prefix: route.prefix}
	st.RxBytes = readUintSysfs(sysfsNet(route.dev, "statistics/rx_bytes"))
	st.TxBytes = readUintSysfs(sysfsNet(route.dev, "statistics/tx_bytes"))

	if c.IsWireless(route.dev) {
		st.Type = "wifi"
		link := c.iwLink(ctx, route.dev)
		st.SSID = iwSSID(link)
		if v, ok := iwInt(link, "signal:"); ok {
			st.SignalDbm = &v
		}
		if v, ok := iwFloat(link, "freq:"); ok {
			st.FreqMhz = int(v)
		}
		if rate, unit := iwBitrate(link); rate != "" {
			st.Bitrate = strings.TrimSpace(rate + " " + unit)
		}
		if st.SSID == "" {
			// Fall back to the NM connection name like the compact path.
			_, conn := c.nmDevice(ctx, route.dev)
			st.SSID = conn
		}
		if sig := c.wifiSignal(ctx, route.dev); sig != nil {
			st.SignalPct = sig
		}
	} else {
		st.Type = "ethernet"
		if v, err := readIntSysfs(sysfsNet(route.dev, "speed")); err == nil {
			st.SpeedMb = &v
		}
		if b, err := os.ReadFile(sysfsNet(route.dev, "duplex")); err == nil {
			if s := strings.TrimSpace(string(b)); s != "" {
				st.Duplex = s
			}
		}
	}

	router, internet := c.pingSamples(ctx, route.gw)
	st.RouterPingMs = router
	st.InternetPingMs = internet
	return st, nil
}

type routeInfo struct {
	dev    string
	gw     string
	src    string
	prefix int
}

// routeGet resolves the default route, mirroring `ip -j route get`.
func (c *Collector) routeGet(ctx context.Context) routeInfo {
	var ri routeInfo
	if !c.Has("ip") {
		return ri
	}
	out, err := c.run.Run(ctx, "ip", "-j", "route", "get", InternetProbe)
	if err != nil || strings.TrimSpace(out) == "" {
		// Fallback to text parsing when `ip -j` is unavailable.
		return c.routeGetText(ctx)
	}
	var routes []struct {
		Dev     string `json:"dev"`
		Gateway string `json:"gateway"`
		Prefsrc string `json:"prefsrc"`
	}
	if err := json.Unmarshal([]byte(out), &routes); err != nil || len(routes) == 0 {
		return c.routeGetText(ctx)
	}
	ri.dev, ri.gw, ri.src = routes[0].Dev, routes[0].Gateway, routes[0].Prefsrc
	if ri.dev != "" {
		ri.prefix = c.inetPrefix(ctx, ri.dev)
	}
	return ri
}

func (c *Collector) routeGetText(ctx context.Context) routeInfo {
	var ri routeInfo
	out, err := c.run.Run(ctx, "ip", "route", "get", InternetProbe)
	if err != nil {
		return ri
	}
	fields := strings.Fields(out)
	for i, f := range fields {
		switch f {
		case "dev":
			if i+1 < len(fields) {
				ri.dev = fields[i+1]
			}
		case "via":
			if i+1 < len(fields) {
				ri.gw = fields[i+1]
			}
		case "src":
			if i+1 < len(fields) {
				ri.src = fields[i+1]
			}
		}
	}
	if ri.dev != "" {
		ri.prefix = c.inetPrefix(ctx, ri.dev)
	}
	return ri
}

func (c *Collector) inetPrefix(ctx context.Context, dev string) int {
	out, err := c.run.Run(ctx, "ip", "-j", "addr", "show", dev)
	if err != nil {
		return 0
	}
	var ifaces []struct {
		AddrInfo []struct {
			Family    string `json:"family"`
			Prefixlen int    `json:"prefixlen"`
		} `json:"addr_info"`
	}
	if err := json.Unmarshal([]byte(out), &ifaces); err != nil {
		return 0
	}
	for _, iface := range ifaces {
		for _, a := range iface.AddrInfo {
			if a.Family == "inet" {
				return a.Prefixlen
			}
		}
	}
	return 0
}

// defaultDevice mirrors the script's `ip route get ... dev` extraction.
func (c *Collector) defaultDevice(ctx context.Context) string {
	return c.routeGet(ctx).dev
}

// nmDevice returns (GENERAL.STATE, GENERAL.CONNECTION) for dev.
func (c *Collector) nmDevice(ctx context.Context, dev string) (state, conn string) {
	if !c.Has("nmcli") {
		return "", ""
	}
	out, err := c.run.Run(ctx, "nmcli", "-t", "-f", "GENERAL.STATE,GENERAL.CONNECTION", "dev", "show", dev)
	if err != nil {
		return "", ""
	}
	for _, line := range strings.Split(out, "\n") {
		if v, ok := strings.CutPrefix(line, "GENERAL.STATE:"); ok {
			state = strings.TrimSpace(v)
		} else if v, ok := strings.CutPrefix(line, "GENERAL.CONNECTION:"); ok {
			conn = strings.TrimSpace(v)
		}
	}
	if conn == "--" {
		conn = ""
	}
	return state, conn
}

// wifiSignal returns the in-use AP's SIGNAL (0-100), like the script's awk.
func (c *Collector) wifiSignal(ctx context.Context, dev string) *int {
	if !c.Has("nmcli") {
		return nil
	}
	out, err := c.run.Run(ctx, "nmcli", "-t", "-f", "IN-USE,SIGNAL", "dev", "wifi", "list", "ifname", dev, "--rescan", "no")
	if err != nil {
		return nil
	}
	for _, line := range strings.Split(out, "\n") {
		inUse, sig, ok := strings.Cut(line, ":")
		if !ok || strings.TrimSpace(inUse) != "*" {
			continue
		}
		if v, err := strconv.Atoi(strings.TrimSpace(sig)); err == nil {
			return &v
		}
	}
	return nil
}

func (c *Collector) iwLink(ctx context.Context, dev string) string {
	if !c.Has("iw") {
		return ""
	}
	out, err := c.run.Run(ctx, "iw", "dev", dev, "link")
	if err != nil {
		return ""
	}
	return out
}

// pingSamples pings the gateway and the internet probe in parallel, like
// print_ping_samples. Without the ping tool or on failure the samples are
// nil (previously: empty QML strings rendered as "--").
func (c *Collector) pingSamples(ctx context.Context, gateway string) (router, internet *float64) {
	if !c.Has("ping") {
		return nil, nil
	}
	type res struct {
		router bool
		v      *float64
	}
	ch := make(chan res, 2)
	if gateway != "" {
		go func() { ch <- res{true, pingOnce(ctx, c.run, gateway)} }()
	}
	go func() { ch <- res{false, pingOnce(ctx, c.run, InternetProbe)} }()
	n := 1
	if gateway != "" {
		n = 2
	}
	for range n {
		r := <-ch
		if r.router {
			router = r.v
		} else {
			internet = r.v
		}
	}
	return router, internet
}

// pingOnce runs `ping -n -c1 -W1 host` and extracts time=.
func pingOnce(ctx context.Context, r Runner, host string) *float64 {
	out, err := r.Run(ctx, "ping", "-n", "-c", "1", "-W", "1", host)
	if err != nil || out == "" {
		return nil
	}
	for _, token := range strings.Fields(out) {
		if strings.HasPrefix(token, "time=") {
			if v, err := strconv.ParseFloat(strings.TrimPrefix(token, "time="), 64); err == nil {
				return &v
			}
		}
	}
	return nil
}

func isWireless(dev string) bool {
	fi, err := os.Stat(filepath.Join("/sys/class/net", dev, "wireless"))
	return err == nil && fi.IsDir()
}

func sysfsNet(dev, leaf string) string {
	return filepath.Join("/sys/class/net", dev, leaf)
}

func readUintSysfs(path string) uint64 {
	b, err := os.ReadFile(path)
	if err != nil {
		return 0
	}
	v, _ := strconv.ParseUint(strings.TrimSpace(string(b)), 10, 64)
	return v
}

func readIntSysfs(path string) (int, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return 0, err
	}
	return strconv.Atoi(strings.TrimSpace(string(b)))
}

// iwSSID extracts the SSID (rest of line, so ':' in SSIDs is safe).
func iwSSID(link string) string {
	for _, line := range strings.Split(link, "\n") {
		if i := strings.Index(line, "SSID:"); i >= 0 {
			return strings.TrimSpace(line[i+len("SSID:"):])
		}
	}
	return ""
}

func iwInt(link, key string) (int, bool) {
	for _, line := range strings.Split(link, "\n") {
		line = strings.TrimSpace(line)
		if !strings.HasPrefix(line, key) {
			continue
		}
		fields := strings.Fields(strings.TrimPrefix(line, key))
		if len(fields) == 0 {
			return 0, false
		}
		v, err := strconv.Atoi(fields[0])
		if err != nil {
			return 0, false
		}
		return v, true
	}
	return 0, false
}

func iwFloat(link, key string) (float64, bool) {
	for _, line := range strings.Split(link, "\n") {
		line = strings.TrimSpace(line)
		if !strings.HasPrefix(line, key) {
			continue
		}
		fields := strings.Fields(strings.TrimPrefix(line, key))
		if len(fields) == 0 {
			return 0, false
		}
		v, err := strconv.ParseFloat(fields[0], 64)
		if err != nil {
			return 0, false
		}
		return v, true
	}
	return 0, false
}

// iwBitrate extracts "tx bitrate: <rate> <unit>".
func iwBitrate(link string) (rate, unit string) {
	for _, line := range strings.Split(link, "\n") {
		line = strings.TrimSpace(line)
		if v, ok := strings.CutPrefix(line, "tx bitrate:"); ok {
			fields := strings.Fields(strings.TrimSpace(v))
			if len(fields) >= 1 {
				rate = fields[0]
			}
			if len(fields) >= 2 {
				unit = fields[1]
			}
			return rate, unit
		}
	}
	return "", ""
}

// ThroughputTracker derives B/s rates from consecutive counter samples,
// replacing NetworkModel.js throughputState. It resets on interface change
// or first sample (previously: prevSampleTime === 0 branch).
type ThroughputTracker struct {
	mu       sync.Mutex
	iface    string
	rx, tx   uint64
	at       int64 // unix nanos; 0 = no sample yet
	down, up float64
}

// NewThroughputTracker builds an empty tracker.
func NewThroughputTracker() *ThroughputTracker { return &ThroughputTracker{} }

// Update folds in a new counter sample and returns (download, upload) B/s.
func (t *ThroughputTracker) Update(iface string, rx, tx uint64, now time.Time) (float64, float64) {
	return t.UpdateNanos(iface, rx, tx, now.UnixNano())
}

// UpdateNanos is Update with an explicit timestamp (test-friendly).
func (t *ThroughputTracker) UpdateNanos(iface string, rx, tx uint64, nowNanos int64) (float64, float64) {
	t.mu.Lock()
	defer t.mu.Unlock()
	if iface != t.iface || t.at == 0 {
		t.iface, t.rx, t.tx, t.at = iface, rx, tx, nowNanos
		t.down, t.up = 0, 0
		return 0, 0
	}
	dt := float64(nowNanos-t.at) / 1e9
	if dt > 0 {
		t.down = clampNonNeg(float64(rx)-float64(t.rx)) / dt
		t.up = clampNonNeg(float64(tx)-float64(t.tx)) / dt
		t.rx, t.tx, t.at = rx, tx, nowNanos
	}
	return t.down, t.up
}

func clampNonNeg(v float64) float64 {
	if v < 0 {
		return 0
	}
	return v
}

// Ping windows mirror NetworkModel.js: a 24-sample history per interface
// with averages over the last 5. A nil sample (timeout, no gateway) is kept
// as a loss entry, exactly like QML's null entries.
const (
	PingHistoryWindow = 24
	PingAverageWindow = 5
)

// PingStats aggregates one interface's ping history.
type PingStats struct {
	RouterAvg       *float64
	InternetAvg     *float64
	InternetLossPct int
	Samples         int
}

// PingTracker maintains per-interface ping histories, replacing
// pingLatencyState in QML. It resets when the interface changes.
type PingTracker struct {
	mu       sync.Mutex
	iface    string
	router   []*float64
	internet []*float64
}

// NewPingTracker builds an empty tracker.
func NewPingTracker() *PingTracker { return &PingTracker{} }

// Update folds in one poll's samples and returns the aggregates.
func (t *PingTracker) Update(iface string, router, internet *float64) PingStats {
	t.mu.Lock()
	defer t.mu.Unlock()
	if iface != t.iface {
		t.iface = iface
		t.router = nil
		t.internet = nil
	}
	t.router = appendPingSample(t.router, router)
	t.internet = appendPingSample(t.internet, internet)
	return PingStats{
		RouterAvg:       averagePing(t.router),
		InternetAvg:     averagePing(t.internet),
		InternetLossPct: lossPct(t.internet),
		Samples:         len(t.internet),
	}
}

func appendPingSample(samples []*float64, v *float64) []*float64 {
	samples = append(samples, v)
	if len(samples) > PingHistoryWindow {
		samples = append([]*float64{}, samples[len(samples)-PingHistoryWindow:]...)
	}
	return samples
}

// averagePing averages the last PingAverageWindow samples, skipping losses.
// It returns nil when no sample succeeded (QML renders "Timeout").
func averagePing(samples []*float64) *float64 {
	start := len(samples) - PingAverageWindow
	if start < 0 {
		start = 0
	}
	var total float64
	var n int
	for _, v := range samples[start:] {
		if v == nil {
			continue
		}
		total += *v
		n++
	}
	if n == 0 {
		return nil
	}
	avg := total / float64(n)
	return &avg
}

// lossPct is the share of timed-out samples, rounded like QML.
func lossPct(samples []*float64) int {
	if len(samples) == 0 {
		return 0
	}
	var lost int
	for _, v := range samples {
		if v == nil {
			lost++
		}
	}
	return int(float64(lost)/float64(len(samples))*100 + 0.5)
}
