package power

import (
	"context"
	"strings"
	"testing"
)

type powerCall struct {
	name string
	args []string
}

func stubPower(calls *[]powerCall) runFunc {
	return func(_ context.Context, name string, args ...string) (string, error) {
		if calls != nil {
			*calls = append(*calls, powerCall{name, args})
		}
		return "", nil
	}
}

func TestPowerUsage(t *testing.T) {
	for _, args := range [][]string{nil, {}, {"logout", "extra"}, {"suspend"}, {"bogus"}} {
		if _, err := NewHandlerWithRunner(stubPower(nil)).Exec(context.Background(), args); err == nil {
			t.Fatalf("want usage error for %v", args)
		}
	}
}

func TestPowerCommands(t *testing.T) {
	cases := map[string]string{
		"logout":   "hyprctl dispatch exit",
		"lock":     "hyprlock",
		"restart":  "systemctl reboot",
		"shutdown": "systemctl poweroff",
	}
	for action, want := range cases {
		var calls []powerCall
		res, err := NewHandlerWithRunner(stubPower(&calls)).Exec(context.Background(), []string{action})
		if err != nil || res.Message == "" {
			t.Fatalf("%s: got %v %v", action, res, err)
		}
		if len(calls) != 1 || strings.TrimSpace(calls[0].name+" "+strings.Join(calls[0].args, " ")) != want {
			t.Fatalf("%s: got %v", action, calls)
		}
	}
}
