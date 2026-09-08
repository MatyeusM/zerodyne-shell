package audio

import (
	"context"
	"strings"
)

// ResolveVolumeSink follows a DSP sink down to the physical sink whose
// volume and mute the output really uses, ported from
// omarchy-audio-output-sink. A DSP sink (speaker tuning filter-chain, or
// EasyEffects) can be the selected output without being where loudness
// lives; changing its volume alters the level going into the processing.
//
// With no DSP in the path the name resolves to itself. Generic logic
// only: the `omarchy-audio-tuning fronted-sink` dependency is out of
// scope, so a tuning-fronted physical sink is not hidden here.
func ResolveVolumeSink(ctx context.Context, run RunFunc, sink string) string {
	sink = strings.TrimSpace(sink)
	if sink == "" || strings.HasPrefix(sink, "alsa_") {
		return sink
	}
	inputs, err := run(ctx, "pactl", "list", "sink-inputs")
	if err != nil {
		return sink
	}
	downstream := followDownstream(inputs, sink)
	if downstream == "" {
		// Idle and unlinked: keep the sink itself so callers still have
		// something to act on.
		return sink
	}
	if name := sinkNameForID(ctx, run, downstream); name != "" {
		return name
	}
	return sink
}

// followDownstream scans `pactl list sink-inputs` for the stream a
// virtual sink feeds through, returning the sink id underneath. A stream
// whose node.name starts with the virtual sink's name is its downstream
// link; the EasyEffects output stream matches by application name.
func followDownstream(inputs, virt string) string {
	target := ""
	lines := strings.Split(inputs, "\n")
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "Sink Input #") {
			target = ""
			continue
		}
		if strings.HasPrefix(trimmed, "Sink:") {
			if f := strings.Fields(trimmed); len(f) >= 2 {
				target = f[1]
			}
			continue
		}
		if target == "" {
			continue
		}
		if strings.Contains(line, "node.name = ") {
			name := line
			if i := strings.Index(name, `node.name = "`); i >= 0 {
				name = name[i+len(`node.name = "`):]
				name = strings.TrimSuffix(name, `"`)
				if strings.HasPrefix(name, virt) {
					return target
				}
			}
		}
		if virt == "easyeffects_sink" && strings.Contains(line, `application.name = "EasyEffects"`) {
			return target
		}
	}
	return ""
}

// sinkNameForID maps a numeric sink id to its name via
// `pactl list sinks short` ("<id> <name> ...").
func sinkNameForID(ctx context.Context, run RunFunc, id string) string {
	out, err := run(ctx, "pactl", "list", "sinks", "short")
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(out, "\n") {
		if f := strings.Fields(line); len(f) >= 2 && f[0] == id {
			return f[1]
		}
	}
	return ""
}

// ClassifySink maps a sink's name/description/properties to a device
// class plus the bluetooth flag. A substring ladder reimplemented from
// omarchy Model.js isHeadphones/sinkGlyph (no code copied): order
// matters, headphone before phone, and bluetooth never overrides the
// physical kind.
func ClassifySink(name, desc string, props map[string]string) (class string, bluetooth bool) {
	blob := classBlob(name, desc, props)
	switch {
	case hasAny(blob, "headphone", "earbud", "earphone", "airpod"):
		class = ClassHeadphones
	case hasAny(blob, "headset", "hands-free", "handsfree"):
		class = ClassHeadset
	case hasAny(blob, "hdmi", "display"):
		class = ClassHDMI
	case hasAny(blob, "car"):
		class = ClassCar
	case strings.Contains(blob, "phone"):
		class = ClassPhone
	default:
		class = ClassSpeaker
	}
	bluetooth = hasAny(blob, "bluetooth", "bluez", "a2dp")
	return class, bluetooth
}

// ClassifySource maps a source to a device class. Reimplemented from
// omarchy Model.js sourceGlyph: headset art for headset-likes, camera
// for webcams, microphone for the rest.
func ClassifySource(name, desc string, props map[string]string) string {
	blob := classBlob(name, desc, props)
	switch {
	case hasAny(blob, "headset", "headphone", "earbud", "earphone", "airpod", "hands-free", "handsfree"):
		return ClassHeadset
	case hasAny(blob, "webcam", "camera"):
		return ClassCamera
	case hasAny(blob, "phone"):
		return ClassPhone
	default:
		return ClassMicrophone
	}
}

// classBlob joins the identifying strings into one lowercase blob. Extra
// property keys (bus, form factor, bluez icon) are included so wireless
// sinks classify even when their name is a bare bluez_output id.
func classBlob(name, desc string, props map[string]string) string {
	parts := []string{name, desc}
	for _, k := range []string{
		"device.icon-name", "device.icon_name", "device.product.name",
		"device.product.name", "node.description", "node.nick",
		"device.bus", "device.form_factor", "api.bluez5.icon",
	} {
		if v := props[k]; v != "" {
			parts = append(parts, v)
		}
	}
	return strings.ToLower(strings.Join(parts, " "))
}

func hasAny(blob string, subs ...string) bool {
	for _, s := range subs {
		if strings.Contains(blob, s) {
			return true
		}
	}
	return false
}

// AppSinkInputIDs parses `pactl list sink-inputs` into the ids of real
// application streams, ported from the set-default script's awk filter:
// a DSP filter-chain's own output carries no application.name, and moving
// it would rewire the processing itself (onto headphones, or a cycle).
// EasyEffects' stream stays put for the same reason.
func AppSinkInputIDs(inputs string) []string {
	var ids []string
	id, app := "", ""
	flush := func() {
		if id != "" && app != "" && app != "EasyEffects" {
			ids = append(ids, id)
		}
		id, app = "", ""
	}
	for _, line := range strings.Split(inputs, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "Sink Input #") {
			flush()
			id = strings.TrimPrefix(trimmed, "Sink Input #")
			continue
		}
		if strings.Contains(line, "application.name = ") {
			v := line
			if i := strings.Index(v, `application.name = "`); i >= 0 {
				v = v[i+len(`application.name = "`):]
				app = strings.TrimSuffix(v, `"`)
			}
		}
	}
	flush()
	return ids
}
