package audio

import (
	"context"
	"strings"
	"testing"
)

const switchSinksJSON = `[
 {"index":10,"name":"alsa_output.pci.x","description":"Built-in Audio","mute":false,
  "volume":{"front-left":{"value_percent":"50%"}},"ports":[],
  "properties":{}},
 {"index":20,"name":"bluez_output.y","description":"Headphones","mute":true,
  "volume":{"front-left":{"value_percent":"30%"}},"ports":[{"availability":"available"}],
  "properties":{"device.bus":"bluetooth"}},
 {"index":30,"name":"hdmi_output.z","description":"HDMI","mute":false,
  "volume":{"front-left":{"value_percent":"90%"}},"ports":[{"availability":"not available"}],
  "properties":{}}
]`

func stubSwitch(def string, calls *[]audioCall) runFunc {
	return func(_ context.Context, name string, args ...string) (string, error) {
		if calls != nil {
			*calls = append(*calls, audioCall{name, args})
		}
		key := name + " " + strings.Join(args, " ")
		switch {
		case strings.HasPrefix(key, "pactl get-default-sink"):
			return def, nil
		case strings.HasPrefix(key, "pactl -f json list sinks"):
			return switchSinksJSON, nil
		case strings.HasPrefix(key, "pactl list sink-inputs"):
			return "Sink Input #1\n\tProperties:\n\t\tapplication.name = \"Firefox\"\n", nil
		case strings.HasPrefix(key, "pactl get-sink-volume"):
			return "Volume: front-left: 1 /  30% / -25 dB", nil
		case strings.HasPrefix(key, "pactl get-sink-mute"):
			return "Mute: yes", nil
		}
		return "", nil
	}
}

func TestOutputDefaultMovesApps(t *testing.T) {
	var calls []audioCall
	run := func(_ context.Context, name string, args ...string) (string, error) {
		calls = append(calls, audioCall{name, args})
		if name == "pactl" && strings.Join(args, " ") == "list sink-inputs" {
			return "Sink Input #1\n\tProperties:\n\t\tapplication.name = \"Firefox\"\n" +
				"Sink Input #2\n\tProperties:\n\t\tnode.name = \"chain\"\n" +
				"Sink Input #3\n\tProperties:\n\t\tapplication.name = \"EasyEffects\"\n", nil
		}
		return "", nil
	}
	res, err := NewOutputDefaultHandlerWithRunner(run).Exec(context.Background(), []string{"21", "bluez_output.y"})
	if err != nil || res.Message == "" {
		t.Fatalf("got %v %v", res, err)
	}
	seq := audioSequence(calls)
	for _, want := range []string{
		"wpctl set-default 21",
		"pactl set-default-sink bluez_output.y",
		"pactl move-sink-input 1 bluez_output.y",
	} {
		if !strings.Contains(seq, want) {
			t.Fatalf("missing %q:\n%s", want, seq)
		}
	}
	if strings.Contains(seq, "move-sink-input 2") || strings.Contains(seq, "move-sink-input 3") {
		t.Fatalf("DSP/EasyEffects streams must stay put:\n%s", seq)
	}
}

func TestOutputDefaultUsage(t *testing.T) {
	for _, args := range [][]string{nil, {"only"}, {"a", "b", "c"}, {"", "x"}} {
		if _, err := NewOutputDefaultHandlerWithRunner(stubSwitch("", nil)).Exec(context.Background(), args); err == nil {
			t.Fatalf("want usage error for %v", args)
		}
	}
}

func TestInputDefaultMovesAll(t *testing.T) {
	var calls []audioCall
	run := func(_ context.Context, name string, args ...string) (string, error) {
		calls = append(calls, audioCall{name, args})
		if name == "pactl" && strings.Join(args, " ") == "list short source-outputs" {
			return "5\t0\talsa_input.x\tPipeWire\n6\t0\talsa_input.x\tPipeWire\n", nil
		}
		return "", nil
	}
	if _, err := NewInputDefaultHandlerWithRunner(run).Exec(context.Background(), []string{"50", "alsa_input.y"}); err != nil {
		t.Fatal(err)
	}
	seq := audioSequence(calls)
	for _, want := range []string{
		"wpctl set-default 50",
		"pactl set-default-source alsa_input.y",
		"pactl move-source-output 5 alsa_input.y",
		"pactl move-source-output 6 alsa_input.y",
	} {
		if !strings.Contains(seq, want) {
			t.Fatalf("missing %q:\n%s", want, seq)
		}
	}
}

func TestSwitchNext(t *testing.T) {
	var calls []audioCall
	res, err := NewOutputSwitchHandlerWithRunner(stubSwitch("alsa_output.pci.x", &calls)).Exec(context.Background(), nil)
	if err != nil {
		t.Fatal(err)
	}
	// HDMI is unavailable, so next from the built-in is the bluetooth sink.
	if res.Data["name"] != "bluez_output.y" {
		t.Fatalf("data = %v", res.Data)
	}
	if res.Data["muted"] != true || res.Data["volume_pct"] != 30 {
		t.Fatalf("level read from the effective sink: %v", res.Data)
	}
	seq := audioSequence(calls)
	if !strings.Contains(seq, "pactl set-default-sink bluez_output.y") ||
		!strings.Contains(seq, "wpctl set-default 20") {
		t.Fatalf("got:\n%s", seq)
	}
	if !strings.Contains(seq, "pactl move-sink-input 1 bluez_output.y") {
		t.Fatalf("app streams must follow:\n%s", seq)
	}
}

func TestSwitchPreviousWraps(t *testing.T) {
	res, err := NewOutputSwitchHandlerWithRunner(stubSwitch("alsa_output.pci.x", nil)).Exec(context.Background(), []string{"previous"})
	if err != nil {
		t.Fatal(err)
	}
	// Previous from index 0 wraps to the last available sink.
	if res.Data["name"] != "bluez_output.y" {
		t.Fatalf("data = %v", res.Data)
	}
}

func TestSwitchSameSinkNoSet(t *testing.T) {
	var calls []audioCall
	// Only one available sink: next is itself, so no set-default runs.
	run := func(ctx context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		if strings.HasPrefix(key, "pactl -f json list sinks") {
			return `[{"index":10,"name":"alsa_output.pci.x","description":"Built-in","mute":false,
 "volume":{"front-left":{"value_percent":"50%"}},"ports":[],"properties":{}}]`, nil
		}
		return stubSwitch("alsa_output.pci.x", &calls)(ctx, name, args...)
	}
	res, err := NewOutputSwitchHandlerWithRunner(run).Exec(context.Background(), nil)
	if err != nil || res.Data["name"] != "alsa_output.pci.x" {
		t.Fatalf("got %v %v", res, err)
	}
	if strings.Contains(audioSequence(calls), "set-default-sink") {
		t.Fatalf("no set when already there:\n%s", audioSequence(calls))
	}
}

func TestSwitchUsage(t *testing.T) {
	for _, args := range [][]string{{"next", "extra"}, {"sideways"}} {
		if _, err := NewOutputSwitchHandlerWithRunner(stubSwitch("", nil)).Exec(context.Background(), args); err == nil {
			t.Fatalf("want usage error for %v", args)
		}
	}
}
