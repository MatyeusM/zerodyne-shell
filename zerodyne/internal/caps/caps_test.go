package caps

import (
	"testing"
)

func TestProbeCaches(t *testing.T) {
	a := Probe()
	b := Probe()
	if len(a) == 0 {
		t.Fatal("expected probed tools")
	}
	for _, tool := range Tools {
		if _, ok := a[tool]; !ok {
			t.Fatalf("missing tool %q", tool)
		}
		if a[tool] != b[tool] {
			t.Fatalf("cache inconsistent for %q", tool)
		}
	}
}

func TestAvailableShAndBogus(t *testing.T) {
	if !Available("sh") {
		t.Fatal("expected sh on PATH")
	}
	if Available("zerodyne-definitely-missing-binary") {
		t.Fatal("expected missing binary to be unavailable")
	}
}

func TestRefresh(t *testing.T) {
	m := Refresh()
	if len(m) != len(Tools) {
		t.Fatalf("refresh = %v", m)
	}
}
