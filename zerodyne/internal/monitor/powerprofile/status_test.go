package powerprofile

import (
	"testing"
)

const stubList = `  performance:
    CpuDriver:	amd_pstate
    Degraded:   no

* balanced:
    CpuDriver:	amd_pstate
    PlatformDriver:	placeholder

  power-saver:
    CpuDriver:	amd_pstate
    PlatformDriver:	placeholder
`

func TestParseList(t *testing.T) {
	active, profiles := ParseList(stubList)
	if active != "balanced" {
		t.Fatalf("active = %q", active)
	}
	if len(profiles) != 3 || profiles[0] != "performance" || profiles[2] != "power-saver" {
		t.Fatalf("profiles = %v", profiles)
	}
}

func TestParseListSkipsDetails(t *testing.T) {
	// Detail lines (4+ indent) and unknown names never leak in.
	active, profiles := ParseList("CpuDriver:\n    performance:\n* bogus:\n")
	if active != "" || len(profiles) != 0 {
		t.Fatalf("got %q %v", active, profiles)
	}
}

func TestValidProfile(t *testing.T) {
	for _, p := range []string{"performance", "balanced", "power-saver"} {
		if !ValidProfile(p) {
			t.Fatalf("%q should be valid", p)
		}
	}
	if ValidProfile("turbo") {
		t.Fatal("turbo should be invalid")
	}
}
