package audio

import (
	"context"
	"strings"
	"testing"
)

func TestParseSinkVolume(t *testing.T) {
	v, ok := ParseSinkVolume("Volume: front-left: 24253 /  37% / -25.90 dB,   front-right: 24253 /  37% / -25.90 dB\n        balance 0.00")
	if !ok || v != 37 {
		t.Fatalf("got %v %v", v, ok)
	}
	// Over-amplification stays raw (clamp only on write).
	v, ok = ParseSinkVolume("Volume: front-left: 100% / 125% / ...")
	if !ok || v != 100 {
		t.Fatalf("first token wins, got %v %v", v, ok)
	}
	if _, ok := ParseSinkVolume("Mute: no"); ok {
		t.Fatal("mute output has no volume")
	}
}

func TestParseWpctlVolume(t *testing.T) {
	pct, muted := ParseWpctlVolume("Volume: 1.00")
	if pct != 100 || muted {
		t.Fatalf("got %v %v", pct, muted)
	}
	pct, muted = ParseWpctlVolume("Volume: 0.80 [MUTED]")
	if pct != 80 || !muted {
		t.Fatalf("got %v %v", pct, muted)
	}
}

func TestClassifyLadder(t *testing.T) {
	cases := []struct {
		name, desc string
		props      map[string]string
		class      string
		bt         bool
	}{
		{"bluez_output.1", "WH-1000XM4", map[string]string{"device.icon_name": "audio-headphones-bluetooth", "device.bus": "bluetooth"}, ClassHeadphones, true},
		{"alsa_output.usb-headset", "USB Headset", nil, ClassHeadset, false},
		{"alsa_output.phone", "Phone Audio", nil, ClassPhone, false},
		// "headphones" contains "phone": headphone must win by order.
		{"alsa_output.x", "Studio Headphones", nil, ClassHeadphones, false},
		{"alsa_output.hdmi-stereo", "Navi 31 HDMI", nil, ClassHDMI, false},
		{"bluez_output.car", "Car Kit", map[string]string{"device.bus": "bluetooth"}, ClassCar, true},
		{"alsa_output.pci.analog-stereo", "Built-in Audio", nil, ClassSpeaker, false},
		// Bluetooth flag never overrides the physical kind.
		{"bluez_output.2", "Portable Speaker", map[string]string{"device.bus": "bluetooth"}, ClassSpeaker, true},
	}
	for _, c := range cases {
		class, bt := ClassifySink(c.name, c.desc, c.props)
		if class != c.class || bt != c.bt {
			t.Errorf("%s: got %q %v want %q %v", c.name, class, bt, c.class, c.bt)
		}
	}
}

func TestClassifySource(t *testing.T) {
	if got := ClassifySource("alsa_input.usb-tonor", "TONOR TC30", nil); got != ClassMicrophone {
		t.Fatalf("got %q", got)
	}
	if got := ClassifySource("alsa_input.webcam", "Webcam C920", nil); got != ClassCamera {
		t.Fatalf("got %q", got)
	}
	if got := ClassifySource("bluez_input.x", "Headset Mic", nil); got != ClassHeadset {
		t.Fatalf("got %q", got)
	}
}

func TestResolveVolumeSink(t *testing.T) {
	inputs := `Sink Input #7
		Sink: 43
		Properties:
			node.name = "tuning_sink_output"
Sink Input #9
		Sink: 43
		Properties:
			application.name = "Firefox"
			node.name = "Firefox"
`
	short := "43\talsa_output.pci-0000_00_1f.3.analog-stereo\tPipeWire\n115\ttuning_sink\tPipeWire\n"
	run := func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		if strings.HasPrefix(key, "pactl list sink-inputs") {
			return inputs, nil
		}
		if strings.HasPrefix(key, "pactl list sinks short") {
			return short, nil
		}
		return "", errNoStub
	}
	// Physical sinks resolve to themselves.
	if got := ResolveVolumeSink(context.Background(), run, "alsa_output.pci.x"); got != "alsa_output.pci.x" {
		t.Fatalf("got %q", got)
	}
	// Virtual tuning sink follows its stream down.
	if got := ResolveVolumeSink(context.Background(), run, "tuning_sink"); got != "alsa_output.pci-0000_00_1f.3.analog-stereo" {
		t.Fatalf("got %q", got)
	}
	// Idle and unlinked: fall back to the sink itself.
	if got := ResolveVolumeSink(context.Background(), run, "easyeffects_sink"); got != "easyeffects_sink" {
		t.Fatalf("got %q", got)
	}
}

func TestResolveEasyEffects(t *testing.T) {
	inputs := `Sink Input #3
		Sink: 43
		Properties:
			application.name = "EasyEffects"
			node.name = "easyeffects"
`
	short := "43\talsa_output.pci.x\tPipeWire\n"
	run := func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		if strings.HasPrefix(key, "pactl list sink-inputs") {
			return inputs, nil
		}
		return short, nil
	}
	if got := ResolveVolumeSink(context.Background(), run, "easyeffects_sink"); got != "alsa_output.pci.x" {
		t.Fatalf("got %q", got)
	}
}

func TestAppSinkInputIDs(t *testing.T) {
	inputs := `Sink Input #1
		Properties:
			application.name = "Firefox"
Sink Input #2
		Properties:
			node.name = "tuning_chain"
Sink Input #3
		Properties:
			application.name = "EasyEffects"
`
	ids := AppSinkInputIDs(inputs)
	if len(ids) != 1 || ids[0] != "1" {
		t.Fatalf("got %v", ids)
	}
}

func TestSinkAvailable(t *testing.T) {
	mk := func(av ...string) []struct {
		Availability string `json:"availability"`
	} {
		var out []struct {
			Availability string `json:"availability"`
		}
		for _, a := range av {
			out = append(out, struct {
				Availability string `json:"availability"`
			}{a})
		}
		return out
	}
	if !SinkAvailable(nil) {
		t.Fatal("no ports means available")
	}
	if !SinkAvailable(mk("not available", "available")) {
		t.Fatal("any available port counts")
	}
	if SinkAvailable(mk("not available")) {
		t.Fatal("all-unavailable means unavailable")
	}
}

const stubSinksJSON = `[
 {"index":115,"name":"bluez_output.44_73_D6_A4_73_3F.1","description":"Zone Vibe 100","mute":false,
  "volume":{"front-left":{"value_percent":"37%"}},"ports":[{"availability":"available"}],
  "properties":{"device.icon_name":"audio-headset-bluetooth","device.bus":"bluetooth","device.form_factor":"headset"}},
 {"index":43,"name":"alsa_output.pci-0000_00_1f.3.analog-stereo","description":"Built-in Audio","mute":false,
  "volume":{"front-left":{"value_percent":"80%"}},"ports":[{"availability":"not available"}],
  "properties":{}}
]`

const stubSourcesJSON = `[
 {"index":58,"name":"alsa_input.usb-tonor","description":"TONOR TC30","properties":{}}
]`

type stubErr string

func (e stubErr) Error() string { return string(e) }

const errNoStub = stubErr("no stub")

func stubAudio(files map[string]string) FuncRunner {
	return FuncRunner{Fn: func(_ context.Context, name string, args ...string) (string, error) {
		key := name + " " + strings.Join(args, " ")
		switch {
		case strings.HasPrefix(key, "pactl get-default-sink"):
			return "alsa_output.pci-0000_00_1f.3.analog-stereo", nil
		case strings.HasPrefix(key, "pactl get-sink-volume"):
			return "Volume: front-left: 52428 /  80% / 0.00 dB", nil
		case strings.HasPrefix(key, "pactl get-sink-mute"):
			return "Mute: no", nil
		case strings.HasPrefix(key, "pactl -f json list sinks"):
			return stubSinksJSON, nil
		case strings.HasPrefix(key, "pactl get-default-source"):
			return "alsa_input.usb-tonor", nil
		case strings.HasPrefix(key, "pactl -f json list sources"):
			return stubSourcesJSON, nil
		case strings.HasPrefix(key, "wpctl get-volume"):
			return "Volume: 0.80", nil
		}
		return "", errNoStub
	}}
}

func TestCollectorStatus(t *testing.T) {
	c := NewCollector(stubAudio(nil))
	c.Has = func(string) bool { return true }
	st, err := c.Collect(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if st.VolumePct != 80 || st.Muted {
		t.Fatalf("level: %+v", st)
	}
	if st.VolumeSink != "alsa_output.pci-0000_00_1f.3.analog-stereo" {
		t.Fatalf("volume sink: %q", st.VolumeSink)
	}
	// The bluetooth sink classifies as headset (form factor) with the
	// wireless flag; the unavailable built-in sink stays listed.
	if len(st.Sinks) != 2 {
		t.Fatalf("sinks: %+v", st.Sinks)
	}
	if st.Sinks[0].Class != ClassHeadset || !st.Sinks[0].Bluetooth {
		t.Fatalf("sink0: %+v", st.Sinks[0])
	}
	if st.Sinks[1].Available {
		t.Fatalf("sink1 should be unavailable: %+v", st.Sinks[1])
	}
	if st.Sink.Name != st.VolumeSink {
		t.Fatalf("default sink: %+v", st.Sink)
	}
	if st.Source.Class != ClassMicrophone || !st.Sources[0].IsDefault {
		t.Fatalf("source: %+v", st)
	}
	if st.InputVolumePct != 80 || st.InputMuted {
		t.Fatalf("input: %+v", st)
	}
}

func TestCollectorNoPactl(t *testing.T) {
	c := NewCollector(stubAudio(nil))
	c.Has = func(string) bool { return false }
	if _, err := c.Collect(context.Background()); err == nil {
		t.Fatal("expected error without pactl")
	}
}

func TestMonitorName(t *testing.T) {
	c := NewCollector(stubAudio(nil))
	c.Has = func(string) bool { return true }
	m := NewMonitorWithCollector(c)
	if m.Name() != "audio" {
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
