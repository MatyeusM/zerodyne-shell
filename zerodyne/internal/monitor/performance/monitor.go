// Package performance implements the performance monitor.
//
// The repo has no performance shell scripts yet (see TODO.md Phase 4:
// CPU %, RAM %, temps as live graphs, uptime). This package establishes the
// extension point with a minimal real snapshot — CPU from /proc/stat deltas
// and memory from /proc/meminfo — and leaves graphs, per-core, temps, and
// disk/net details for later migration.
package performance

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"

	"zerodyne/internal/monitor"
)

// Snapshot is the typed performance state.
type Snapshot struct {
	CPUPercent float64 `json:"cpu_percent"`
	// Static CPU identity for panel labels (best-effort; empty when
	// /proc/cpuinfo is unreadable).
	CPUName  string `json:"cpu_name,omitempty"`
	CPUCores int    `json:"cpu_cores,omitempty"`
	// Hottest sysfs thermal/hwmon reading, or nil when no sensor exists.
	CPUTempC    *float64 `json:"cpu_temp_c,omitempty"`
	MemTotalKB  uint64   `json:"mem_total_kb"`
	MemAvailKB  uint64   `json:"mem_avail_kb"`
	MemPercent  float64  `json:"mem_percent"`
	SwapTotalKB uint64   `json:"swap_total_kb,omitempty"`
	SwapFreeKB  uint64   `json:"swap_free_kb,omitempty"`
	SwapPercent float64  `json:"swap_percent,omitempty"`
	SampledAt   int64    `json:"sampled_at_unix"`
}

// Monitor is the registered "performance" monitor.
type Monitor struct {
	monitor.Base
	mu       sync.Mutex
	prevIdle uint64
	prevTot  uint64
	ready    bool
}

// NewMonitor builds the performance monitor.
func NewMonitor() *Monitor { return &Monitor{} }

// Name implements monitor.Monitor.
func (m *Monitor) Name() string { return "performance" }

// Snapshot implements monitor.Monitor.
func (m *Monitor) Snapshot(ctx context.Context) (any, error) {
	_ = ctx
	m.mu.Lock()
	defer m.mu.Unlock()
	idle, total, err := readCPU()
	if err != nil {
		return nil, err
	}
	cpu := 0.0
	if m.ready && total > m.prevTot {
		idleD := float64(idle - m.prevIdle)
		totalD := float64(total - m.prevTot)
		if totalD > 0 && idleD <= totalD {
			cpu = (1 - idleD/totalD) * 100
		}
	}
	m.prevIdle, m.prevTot, m.ready = idle, total, true

	totalKB, availKB, swapTotKB, swapFreeKB, err := readMem()
	if err != nil {
		return nil, err
	}
	memPct := 0.0
	if totalKB > 0 {
		memPct = float64(totalKB-availKB) / float64(totalKB) * 100
	}
	swapPct := 0.0
	if swapTotKB > 0 {
		swapPct = float64(swapTotKB-swapFreeKB) / float64(swapTotKB) * 100
	}
	name, cores := readCPUInfo()
	return Snapshot{
		CPUPercent:  cpu,
		CPUName:     name,
		CPUCores:    cores,
		CPUTempC:    readCPUTemp(),
		MemTotalKB:  totalKB,
		MemAvailKB:  availKB,
		MemPercent:  memPct,
		SwapTotalKB: swapTotKB,
		SwapFreeKB:  swapFreeKB,
		SwapPercent: swapPct,
		SampledAt:   time.Now().Unix(),
	}, nil
}

// readCPU parses the aggregate "cpu" line of /proc/stat.
func readCPU() (idle, total uint64, err error) {
	f, err := os.Open("/proc/stat")
	if err != nil {
		return 0, 0, fmt.Errorf("performance: /proc/stat: %w", err)
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		fields := strings.Fields(sc.Text())
		if len(fields) < 5 || fields[0] != "cpu" {
			continue
		}
		var vals []uint64
		for _, s := range fields[1:] {
			v, err := strconv.ParseUint(s, 10, 64)
			if err != nil {
				return 0, 0, fmt.Errorf("performance: parse /proc/stat: %w", err)
			}
			vals = append(vals, v)
		}
		for _, v := range vals {
			total += v
		}
		// idle + iowait
		if len(vals) >= 4 {
			idle = vals[3] + vals[4]
		} else if len(vals) >= 4 {
			idle = vals[3]
		}
		return idle, total, nil
	}
	return 0, 0, fmt.Errorf("performance: no cpu line in /proc/stat")
}

// readMem parses MemTotal/MemAvailable/SwapTotal/SwapFree from
// /proc/meminfo (kB).
func readMem() (total, avail, swapTot, swapFree uint64, err error) {
	f, err := os.Open("/proc/meminfo")
	if err != nil {
		return 0, 0, 0, 0, fmt.Errorf("performance: /proc/meminfo: %w", err)
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		name, val, ok := strings.Cut(sc.Text(), ":")
		if !ok {
			continue
		}
		n, _ := strconv.ParseUint(strings.Fields(strings.TrimSpace(val))[0], 10, 64)
		switch strings.TrimSpace(name) {
		case "MemTotal":
			total = n
		case "MemAvailable":
			avail = n
		case "SwapTotal":
			swapTot = n
		case "SwapFree":
			swapFree = n
		}
	}
	if total == 0 {
		return 0, 0, 0, 0, fmt.Errorf("performance: no MemTotal in /proc/meminfo")
	}
	return total, avail, swapTot, swapFree, nil
}

// readCPUInfo returns the first model name and the processor count from
// /proc/cpuinfo. Best-effort: empty/zero when unreadable. The redundant
// "<n>-Core Processor" suffix ("AMD Ryzen 5 7600 6-Core Processor") is
// stripped: the core count already has its own field and panel column.
func readCPUInfo() (name string, cores int) {
	f, err := os.Open("/proc/cpuinfo")
	if err != nil {
		return "", 0
	}
	defer f.Close()
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := sc.Text()
		if strings.HasPrefix(line, "processor") {
			cores++
			continue
		}
		if name == "" && strings.HasPrefix(line, "model name") {
			if _, v, ok := strings.Cut(line, ":"); ok {
				name = cpuNameRe.ReplaceAllString(strings.TrimSpace(v), "")
			}
		}
	}
	return name, cores
}

// cpuNameRe matches the redundant core-count suffix in model names.
var cpuNameRe = regexp.MustCompile(`\s*\d+-Core\s+Processor\s*$`)

// readCPUTemp returns the hottest sysfs temperature reading in °C: the
// max over thermal zones plus hwmon temp inputs (k10temp on AMD,
// coretemp on Intel). Nil when no sensor exists.
func readCPUTemp() *float64 {
	var best *float64
	considerMillidegree := func(raw string) {
		v, err := strconv.ParseFloat(strings.TrimSpace(raw), 64)
		if err != nil || v <= 0 {
			return
		}
		c := v / 1000
		if best == nil || c > *best {
			best = &c
		}
	}
	zones, _ := filepath.Glob("/sys/class/thermal/thermal_zone*/temp")
	for _, z := range zones {
		if b, err := os.ReadFile(z); err == nil {
			considerMillidegree(string(b))
		}
	}
	hwmons, _ := filepath.Glob("/sys/class/hwmon/hwmon*/temp*_input")
	for _, h := range hwmons {
		if b, err := os.ReadFile(h); err == nil {
			considerMillidegree(string(b))
		}
	}
	return best
}
