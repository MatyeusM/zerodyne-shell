package powerprofile

import (
	"context"
	"strings"
	"testing"
)

func TestSetUsage(t *testing.T) {
	run := func(context.Context, string, ...string) (string, error) { return "", nil }
	for _, args := range [][]string{nil, {}, {"balanced", "extra"}, {"turbo"}, {"Performance"}} {
		if _, err := NewSetHandlerWithRunner(run).Exec(context.Background(), args); err == nil {
			t.Fatalf("want usage error for %v", args)
		}
	}
}

func TestSetCommand(t *testing.T) {
	var calls []string
	run := func(_ context.Context, name string, args ...string) (string, error) {
		calls = append(calls, name+" "+strings.Join(args, " "))
		return "", nil
	}
	res, err := NewSetHandlerWithRunner(run).Exec(context.Background(), []string{"power-saver"})
	if err != nil || res.Message != "power-saver" {
		t.Fatalf("got %v %v", res, err)
	}
	if len(calls) != 1 || calls[0] != "powerprofilesctl set power-saver" {
		t.Fatalf("calls = %v", calls)
	}
}
