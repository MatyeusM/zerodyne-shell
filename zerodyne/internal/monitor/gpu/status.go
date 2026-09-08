package gpu

import (
	"context"
	"encoding/json"
	"fmt"

	"zerodyne/internal/caps"
)

// Status is the single GPU status shape: the first GPU (`-g 0`).
type Status struct {
	Name       string  `json:"name"`
	UtilPct    float64 `json:"util_pct"`
	TempC      float64 `json:"temp_c"`
	PowerW     float64 `json:"power_w"`
	VramUsedMB float64 `json:"vram_used_mb"`
	VramTotalMB float64 `json:"vram_total_mb"`
	VramPct    float64 `json:"vram_pct"`
}

// Collector gathers GPU state through a Runner.
type Collector struct {
	run Runner
	// Has reports whether a tool is available. Defaults to the cached
	// caps probe; tests stub it.
	Has func(tool string) bool
}

// NewCollector builds a Collector. A nil runner means live system access.
func NewCollector(r Runner) *Collector {
	if r == nil {
		r = DefaultRunner{}
	}
	return &Collector{run: r, Has: caps.Available}
}

// Collect snapshots the first GPU. Without amd-smi it errors and QML
// renders the degraded path.
func (c *Collector) Collect(ctx context.Context) (*Status, error) {
	if !c.Has("amd-smi") {
		return nil, fmt.Errorf("gpu: amd-smi not available")
	}
	out, err := c.run.Run(ctx, "amd-smi", "metric", "-g", "0", "--json")
	if err != nil {
		return nil, fmt.Errorf("gpu: metric: %w", err)
	}
	st, err := parseMetric(out)
	if err != nil {
		return nil, err
	 }
	st.Name = c.gpuName(ctx)
	return st, nil
}

// gpuName resolves the marketing name via `amd-smi static`. Best-effort:
// empty when the call fails.
func (c *Collector) gpuName(ctx context.Context) string {
	out, err := c.run.Run(ctx, "amd-smi", "static", "-g", "0", "--json")
	if err != nil {
		return ""
	}
	var doc struct {
		Data []struct {
			Asic struct {
				MarketName string `json:"market_name"`
			} `json:"asic"`
		} `json:"gpu_data"`
	}
	if err := json.Unmarshal([]byte(out), &doc); err != nil || len(doc.Data) == 0 {
		return ""
	}
	return doc.Data[0].Asic.MarketName
}

// metricDoc is the subset of `amd-smi metric --json` we read. Value
// fields decode number-or-"N/A" via measure.
type metricDoc struct {
	Data []struct {
		Usage struct {
			Gfx measure `json:"gfx_activity"`
		} `json:"usage"`
		Power struct {
			Socket measure `json:"socket_power"`
		} `json:"power"`
		Temperature struct {
			Edge measure `json:"edge"`
		} `json:"temperature"`
		MemUsage struct {
			UsedVram  measure `json:"used_vram"`
			TotalVram measure `json:"total_vram"`
		} `json:"mem_usage"`
	} `json:"gpu_data"`
}

// measure decodes {"value": <number|"N/A">, "unit": ...} into a float,
// with "N/A" (and anything unparseable) reading as zero.
type measure struct {
	Value float64
}

// UnmarshalJSON implements json.Unmarshaler.
func (m *measure) UnmarshalJSON(b []byte) error {
	var raw struct {
		Value any `json:"value"`
	}
	if err := json.Unmarshal(b, &raw); err != nil {
		return err
	}
	m.Value = numValue(raw.Value)
	return nil
}

func numValue(v any) float64 {
	switch n := v.(type) {
	case float64:
		return n
	case int:
		return float64(n)
	case json.Number:
		f, _ := n.Float64()
		return f
	default:
		return 0
	}
}

func parseMetric(out string) (*Status, error) {
	var doc metricDoc
	if err := json.Unmarshal([]byte(out), &doc); err != nil {
		return nil, fmt.Errorf("gpu: metric: %w", err)
	}
	if len(doc.Data) == 0 {
		return nil, fmt.Errorf("gpu: no gpu_data in metric output")
	}
	g := doc.Data[0]
	st := &Status{
		UtilPct:     g.Usage.Gfx.Value,
		TempC:       g.Temperature.Edge.Value,
		PowerW:      g.Power.Socket.Value,
		VramUsedMB:  g.MemUsage.UsedVram.Value,
		VramTotalMB: g.MemUsage.TotalVram.Value,
	}
	if st.VramTotalMB > 0 {
		st.VramPct = st.VramUsedMB / st.VramTotalMB * 100
	}
	return st, nil
}
