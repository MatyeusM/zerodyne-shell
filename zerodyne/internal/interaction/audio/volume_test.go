package audio

import (
	"context"
	"fmt"
	"strings"
	"testing"
)

type audioCall struct {
	name string
	args []string
}

func audioSequence(calls []audioCall) string {
	var parts []string
	for _, c := range calls {
		parts = append(parts, c.name+" "+strings.Join(c.args, " "))
	}
	return strings.Join(parts, "\n")
}

// stubAudioVolume answers a 40% unmuted alsa sink.
func stubAudioVolume(vol int, muted bool, calls *[]audioCall) runFunc {
	return func(_ context.Context, name string, args ...string) (string, error) {
		if calls != nil {
			*calls = append(*calls, audioCall{name, args})
		}
		key := name + " " + strings.Join(args, " ")
		switch {
		case strings.HasPrefix(key, "pactl get-default-sink"):
			return "alsa_output.pci.x", nil
		case strings.HasPrefix(key, "pactl get-sink-volume"):
			return fmt.Sprintf("Volume: front-left: 1 /  %d%% / -25 dB", vol), nil
		case strings.HasPrefix(key, "pactl get-sink-mute"):
			if muted {
				return "Mute: yes", nil
			}
			return "Mute: no", nil
		}
		return "", nil
	}
}

func TestVolumeUsage(t *testing.T) {
	for _, args := range [][]string{nil, {}, {"raise", "extra"}, {"bogus"}, {"+"}, {"+x"}} {
		if _, err := NewVolumeHandlerWithRunner(stubAudioVolume(40, false, nil)).Exec(context.Background(), args); err == nil {
			t.Fatalf("want usage error for %v", args)
		}
	}
}

func TestVolumeRaise(t *testing.T) {
	var calls []audioCall
	res, err := NewVolumeHandlerWithRunner(stubAudioVolume(40, false, &calls)).Exec(context.Background(), []string{"raise"})
	if err != nil {
		t.Fatal(err)
	}
	if res.Message != "45%" {
		t.Fatalf("message = %q", res.Message)
	}
	seq := audioSequence(calls)
	// Any +/- clears mute first (unmute-via-raise), then sets the level.
	mute := strings.Index(seq, "pactl set-sink-mute alsa_output.pci.x 0")
	set := strings.Index(seq, "pactl set-sink-volume alsa_output.pci.x 45%")
	if mute < 0 || set < 0 || mute > set {
		t.Fatalf("want unmute then 45%%:\n%s", seq)
	}
}

func TestVolumeLowerClamps(t *testing.T) {
	var calls []audioCall
	res, err := NewVolumeHandlerWithRunner(stubAudioVolume(3, false, &calls)).Exec(context.Background(), []string{"-5"})
	if err != nil || res.Message != "0%" {
		t.Fatalf("got %v %v", res, err)
	}
	if !strings.Contains(audioSequence(calls), "set-sink-volume alsa_output.pci.x 0%") {
		t.Fatalf("want clamp to 0:\n%s", audioSequence(calls))
	}
}

func TestVolumeRaiseClamps(t *testing.T) {
	res, err := NewVolumeHandlerWithRunner(stubAudioVolume(99, false, nil)).Exec(context.Background(), []string{"+10"})
	if err != nil || res.Message != "100%" {
		t.Fatalf("got %v %v", res, err)
	}
}

func TestVolumeMuteToggle(t *testing.T) {
	var calls []audioCall
	if _, err := NewVolumeHandlerWithRunner(stubAudioVolume(40, false, &calls)).Exec(context.Background(), []string{"mute-toggle"}); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(audioSequence(calls), "pactl set-sink-mute alsa_output.pci.x toggle") {
		t.Fatalf("got:\n%s", audioSequence(calls))
	}
}

func TestVolumeFollowsDSP(t *testing.T) {
	var calls []audioCall
	run := stubAudioVolume(40, false, &calls)
	wrapped := func(ctx context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		if strings.HasPrefix(key, "pactl get-default-sink") {
			return "tuning_sink", nil
		}
		if strings.HasPrefix(key, "pactl list sink-inputs") {
			return "Sink Input #7\n\tSink: 43\n\tProperties:\n\t\tnode.name = \"tuning_sink_out\"\n", nil
		}
		if strings.HasPrefix(key, "pactl list sinks short") {
			return "43\talsa_output.pci.x\tPipeWire\n", nil
		}
		return run(ctx, name, args...)
	}
	if _, err := NewVolumeHandlerWithRunner(wrapped).Exec(context.Background(), []string{"raise"}); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(audioSequence(calls), "set-sink-volume alsa_output.pci.x 45%") {
		t.Fatalf("must act on the physical sink:\n%s", audioSequence(calls))
	}
}

func TestInputMuteUsage(t *testing.T) {
	for _, args := range [][]string{{"toggle", "extra"}, {"bogus"}} {
		if _, err := NewInputMuteHandlerWithRunner(stubAudioVolume(0, false, nil)).Exec(context.Background(), args); err == nil {
			t.Fatalf("want usage error for %v", args)
		}
	}
}

func TestInputMuteForms(t *testing.T) {
	cases := map[string]string{
		"toggle": "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle",
		"on":     "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ 1",
		"off":    "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ 0",
	}
	for action, want := range cases {
		var calls []audioCall
		run := stubAudioVolume(0, false, &calls)
		args := []string{action}
		if action == "toggle" {
			args = nil // default, no arg
		}
		if _, err := NewInputMuteHandlerWithRunner(run).Exec(context.Background(), args); err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(audioSequence(calls), want) {
			t.Fatalf("%s: got:\n%s", action, audioSequence(calls))
		}
	}
}

func TestInputVolumeDelta(t *testing.T) {
	var calls []audioCall
	run := func(_ context.Context, name string, args ...string) (string, error) {
		calls = append(calls, audioCall{name, args})
		if name == "wpctl" && args[0] == "get-volume" {
			return "Volume: 0.80", nil
		}
		return "", nil
	}
	res, err := NewInputMuteHandlerWithRunner(run).Exec(context.Background(), []string{"+5"})
	if err != nil || res.Message != "85%" {
		t.Fatalf("got %v %v", res, err)
	}
	if !strings.Contains(audioSequence(calls), "wpctl set-volume @DEFAULT_AUDIO_SOURCE@ 85%") {
		t.Fatalf("got:\n%s", audioSequence(calls))
	}
}
