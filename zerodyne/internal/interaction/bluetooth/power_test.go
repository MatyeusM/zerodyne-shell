package bluetooth

import (
	"context"
	"strings"
	"testing"
)

type btCall struct {
	name string
	args []string
}

func stubBt(powered bool, calls *[]btCall) runFunc {
	return func(_ context.Context, name string, args ...string) (string, error) {
		if calls != nil {
			*calls = append(*calls, btCall{name, args})
		}
		if name == "rfkill" {
			return "", nil
		}
		switch args[0] {
		case "list":
			return "Controller AA:BB:CC:DD:EE:FF MATTHIASDESKTOP [default]\n", nil
		case "show":
			if powered {
				return "Controller AA:BB:CC:DD:EE:FF\n\tPowered: yes\n", nil
			}
			return "Controller AA:BB:CC:DD:EE:FF\n\tPowered: no\n", nil
		}
		return "", nil
	}
}

func TestPowerUsage(t *testing.T) {
	for _, args := range [][]string{nil, {}, {"on", "extra"}, {"bogus"}} {
		if _, err := NewPowerHandlerWithRunner(stubBt(true, nil)).Exec(context.Background(), args); err == nil {
			t.Fatalf("want usage error for %v", args)
		}
	}
}

func TestPowerOnUnblocks(t *testing.T) {
	var calls []btCall
	res, err := NewPowerHandlerWithRunner(stubBt(true, &calls)).Exec(context.Background(), []string{"on"})
	if err != nil || res.Message == "" {
		t.Fatalf("exec: %v %v", res, err)
	}
	if len(calls) == 0 || calls[0].name != "rfkill" || strings.Join(calls[0].args, " ") != "unblock bluetooth" {
		t.Fatalf("first call should unblock, got %v", calls)
	}
}

func TestPowerOffBlocks(t *testing.T) {
	var calls []btCall
	if _, err := NewPowerHandlerWithRunner(stubBt(true, &calls)).Exec(context.Background(), []string{"off"}); err != nil {
		t.Fatal(err)
	}
	found := false
	for _, c := range calls {
		if c.name == "rfkill" && strings.Join(c.args, " ") == "block bluetooth" {
			found = true
		}
	}
	if !found {
		t.Fatalf("rfkill block missing: %v", calls)
	}
}

func TestPowerToggle(t *testing.T) {
	// Stateful stub: the block is the state, like the real machine.
	blocked := false
	var run runFunc = func(_ context.Context, name string, args ...string) (string, error) {
		if name == "rfkill" {
			if strings.Join(args, " ") == "block bluetooth" {
				blocked = true
			} else {
				blocked = false
			}
			return "", nil
		}
		if name == "bluetoothctl" && len(args) > 0 && args[0] == "list" {
			return "Controller AA:BB:CC:DD:EE:FF MATTHIASDESKTOP [default]\n", nil
		}
		if name == "bluetoothctl" && len(args) > 0 && args[0] == "show" {
			if blocked {
				return "Powered: no\n", nil
			}
			return "Powered: yes\n", nil
		}
		return "", nil
	}
	if _, err := NewPowerHandlerWithRunner(run).Exec(context.Background(), []string{"toggle"}); err != nil {
		t.Fatal(err)
	}
	if !blocked {
		t.Fatal("toggle when on should block")
	}
	if _, err := NewPowerHandlerWithRunner(run).Exec(context.Background(), []string{"toggle"}); err != nil {
		t.Fatal(err)
	}
	if blocked {
		t.Fatal("toggle when off should unblock")
	}
}

func TestPowerIsOn(t *testing.T) {
	res, err := NewPowerHandlerWithRunner(stubBt(true, nil)).Exec(context.Background(), []string{"is-on"})
	if err != nil || res.Message != "on" {
		t.Fatalf("want on, got %v %v", res, err)
	}
	if _, err := NewPowerHandlerWithRunner(stubBt(false, nil)).Exec(context.Background(), []string{"is-on"}); err == nil {
		t.Fatal("want error when off")
	}
}
