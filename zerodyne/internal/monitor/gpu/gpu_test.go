package gpu

import (
	"context"
	"strings"
	"testing"
)

const stubMetric = `{"gpu_data": [{
  "gpu": 0,
  "usage": {"gfx_activity": {"value": 62, "unit": "%"}, "umc_activity": {"value": "N/A", "unit": "%"}},
  "power": {"socket_power": {"value": 120, "unit": "W"}},
  "temperature": {"edge": {"value": 54, "unit": "C"}, "hotspot": {"value": "N/A", "unit": "C"}},
  "mem_usage": {"used_vram": {"value": 2242, "unit": "MB"}, "total_vram": {"value": 16368, "unit": "MB"}}
}]}`

const stubStatic = `{"gpu_data": [{
  "gpu": 0,
  "asic": {"market_name": "AMD Radeon RX 7800 XT"}
}]}`

type stubErr string

func (e stubErr) Error() string { return string(e) }

const errNoStub = stubErr("no stub")

func stubGPU(staticOK bool, calls *[]string) FuncRunner {
	return FuncRunner{Fn: func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		if calls != nil {
			*calls = append(*calls, key)
		}
		if strings.Contains(key, "metric") {
			return stubMetric, nil
		}
		if strings.Contains(key, "static") {
			if !staticOK {
				return "", errNoStub
			}
			return stubStatic, nil
		}
		return "", errNoStub
	}}
}

func TestCollectMetric(t *testing.T) {
	var calls []string
	c := NewCollector(stubGPU(true, &calls))
	c.Has = func(string) bool { return true }
	st, err := c.Collect(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if st.Name != "AMD Radeon RX 7800 XT" {
		t.Fatalf("name = %q", st.Name)
	}
	if st.UtilPct != 62 || st.TempC != 54 || st.PowerW != 120 {
		t.Fatalf("metrics = %+v", st)
	}
	if st.VramUsedMB != 2242 || st.VramTotalMB != 16368 {
		t.Fatalf("vram = %+v", st)
	}
	if got := st.VramUsedMB / st.VramTotalMB * 100; st.VramPct != got {
		t.Fatalf("vram pct = %v", st.VramPct)
	}
	// First GPU only.
	for _, k := range calls {
		if !strings.Contains(k, "-g 0") {
			t.Fatalf("must select -g 0: %q", k)
		}
		if strings.Contains(k, "rocm-smi") {
			t.Fatalf("never rocm-smi: %q", k)
		}
	}
}

func TestCollectStaticBestEffort(t *testing.T) {
	c := NewCollector(stubGPU(false, nil))
	c.Has = func(string) bool { return true }
	st, err := c.Collect(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if st.Name != "" || st.UtilPct != 62 {
		t.Fatalf("metrics survive static failure: %+v", st)
	}
}

func TestCollectNoTool(t *testing.T) {
	c := NewCollector(stubGPU(true, nil))
	c.Has = func(string) bool { return false }
	if _, err := c.Collect(context.Background()); err == nil {
		t.Fatal("expected error without amd-smi")
	}
}

func TestCollectBadJSON(t *testing.T) {
	c := NewCollector(FuncRunner{Fn: func(context.Context, string, ...string) (string, error) {
		return "not json", nil
	}})
	c.Has = func(string) bool { return true }
	if _, err := c.Collect(context.Background()); err == nil {
		t.Fatal("expected JSON error")
	}
}

func TestMonitorName(t *testing.T) {
	c := NewCollector(stubGPU(true, nil))
	c.Has = func(string) bool { return true }
	m := NewMonitorWithCollector(c)
	if m.Name() != "gpu" {
		t.Fatalf("name = %q", m.Name())
	}
	v, err := m.Snapshot(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := v.(*Status); !ok {
		t.Fatalf("type = %T", v)
	}
}
