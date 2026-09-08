package audio

import (
	"context"
	"encoding/json"
	"fmt"
	"strconv"
	"strings"

	"zerodyne/internal/caps"
)

// Device classes. The waybar `portable` class folds into speaker plus the
// bluetooth flag; bluetooth is never a class on its own (the badge, not a
// separate glyph, carries the wireless info).
const (
	ClassHeadphones = "headphones"
	ClassHeadset    = "headset"
	ClassPhone      = "phone"
	ClassSpeaker    = "speaker"
	ClassHDMI       = "hdmi"
	ClassCar        = "car"
	ClassCamera     = "camera"
	ClassMicrophone = "microphone"
	ClassOther      = "other"
)

// SinkInfo describes one sink for display.
type SinkInfo struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Class       string `json:"class"`
	Bluetooth   bool   `json:"bluetooth"`
}

// SinkEntry is one row of the OUTPUT device list. Index is the PipeWire
// object id the set-default ops need (beyond the spec schema, which QML
// cannot call audio-output-default without).
type SinkEntry struct {
	SinkInfo
	Index     int  `json:"index"`
	Available bool `json:"available"`
	IsDefault bool `json:"is_default"`
	VolumePct int  `json:"volume_pct"`
	Muted     bool `json:"muted"`
}

// SourceInfo describes one source for display.
type SourceInfo struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Class       string `json:"class"`
}

// SourceEntry is one row of the INPUT device list.
type SourceEntry struct {
	SourceInfo
	Index     int  `json:"index"`
	IsDefault bool `json:"is_default"`
}

// Status is the single audio status shape: output level plus the device
// lists QML renders. volume_pct may exceed 100 (pactl allows
// over-amplification; waybar shows it raw — clamp only on write).
type Status struct {
	VolumePct      int           `json:"volume_pct"`
	Muted          bool          `json:"muted"`
	VolumeSink     string        `json:"volume_sink"`
	Sink           SinkInfo      `json:"sink"`
	Sinks          []SinkEntry   `json:"sinks"`
	Source         SourceInfo    `json:"source"`
	Sources        []SourceEntry `json:"sources"`
	InputVolumePct int           `json:"input_volume_pct"`
	InputMuted     bool          `json:"input_muted"`
}

// Collector gathers audio state through a Runner.
type Collector struct {
	run Runner
	// Has reports whether a tool is available. Defaults to the cached
	// caps probe; tests stub it.
	Has func(tool string) bool
}

// NewCollector builds a Collector. A nil runner means live system access.
func NewCollector(r Runner) *Collector {
	if r == nil {
		r = DefaultRunner{}
	}
	return &Collector{run: r, Has: caps.Available}
}

// Collect is the one audio status. On total failure (pactl missing) it
// returns an error and QML renders the degraded muted/grey path.
func (c *Collector) Collect(ctx context.Context) (*Status, error) {
	if !c.Has("pactl") {
		return nil, fmt.Errorf("audio: pactl not available")
	}
	run := AsRunFunc(c.run)
	defName := DefaultSink(ctx, run)
	if defName == "" {
		return nil, fmt.Errorf("audio: no default sink")
	}
	volSink := ResolveVolumeSink(ctx, run, defName)
	volPct, err := SinkVolumePct(ctx, run, volSink)
	if err != nil {
		return nil, err
	}
	muted, err := SinkMuted(ctx, run, volSink)
	if err != nil {
		return nil, err
	}
	sinks, err := c.listSinks(ctx, defName)
	if err != nil {
		return nil, err
	}
	st := &Status{
		VolumePct:  volPct,
		Muted:      muted,
		VolumeSink: volSink,
		Sinks:      sinks,
	}
	st.Sink = describeDefault(sinks, defName, volSink)
	c.collectInput(ctx, run, st)
	return st, nil
}

// describeDefault picks the sink row the indicator names: the default
// sink when listed, else the resolved physical sink, else the first row.
func describeDefault(sinks []SinkEntry, defName, volSink string) SinkInfo {
	for _, s := range sinks {
		if s.Name == defName {
			return s.SinkInfo
		}
	}
	for _, s := range sinks {
		if s.Name == volSink {
			return s.SinkInfo
		}
	}
	if len(sinks) > 0 {
		return sinks[0].SinkInfo
	}
	return SinkInfo{Name: defName, Description: defName, Class: ClassOther}
}

// DefaultSink prints the current default sink name, or "".
func DefaultSink(ctx context.Context, run RunFunc) string {
	out, err := run(ctx, "pactl", "get-default-sink")
	if err != nil {
		return ""
	}
	return strings.TrimSpace(out)
}

// DefaultSource prints the current default source name, or "".
func DefaultSource(ctx context.Context, run RunFunc) string {
	out, err := run(ctx, "pactl", "get-default-source")
	if err != nil {
		return ""
	}
	return strings.TrimSpace(out)
}

// SinkVolumePct reads a sink's volume: the first % token on line 1, like
// the scripts' awk. Values above 100 are kept raw.
func SinkVolumePct(ctx context.Context, run RunFunc, sink string) (int, error) {
	out, err := run(ctx, "pactl", "get-sink-volume", sink)
	if err != nil {
		return 0, fmt.Errorf("audio: volume for %s: %w", sink, err)
	}
	v, ok := ParseSinkVolume(out)
	if !ok {
		return 0, fmt.Errorf("audio: could not read volume for %s", sink)
	}
	return v, nil
}

// SinkMuted reports a sink's mute flag (`*yes`, like the scripts).
func SinkMuted(ctx context.Context, run RunFunc, sink string) (bool, error) {
	out, err := run(ctx, "pactl", "get-sink-mute", sink)
	if err != nil {
		return false, fmt.Errorf("audio: mute for %s: %w", sink, err)
	}
	return strings.HasSuffix(strings.TrimSpace(out), "yes"), nil
}

// ParseSinkVolume extracts the first % token on line 1
// ("Volume: front-left: 24253 / 37% / ..."). Pure, for tests and reuse.
func ParseSinkVolume(out string) (int, bool) {
	line := out
	if i := strings.Index(line, "\n"); i >= 0 {
		line = line[:i]
	}
	for _, tok := range strings.Fields(line) {
		if strings.HasSuffix(tok, "%") {
			v, err := strconv.Atoi(strings.TrimSuffix(tok, "%"))
			if err == nil {
				return v, true
			}
		}
	}
	return 0, false
}

// ListSinks returns the full sink list with the default sink name,
// shared by the status snapshot and the output switcher.
func (c *Collector) ListSinks(ctx context.Context) ([]SinkEntry, string, error) {
	defName := DefaultSink(ctx, AsRunFunc(c.run))
	if defName == "" {
		return nil, "", fmt.Errorf("audio: no default sink")
	}
	sinks, err := c.listSinks(ctx, defName)
	if err != nil {
		return nil, "", err
	}
	return sinks, defName, nil
}
// pactlSink is one element of `pactl -f json list sinks`.
type pactlSink struct {
	Index       int    `json:"index"`
	Name        string `json:"name"`
	Description string `json:"description"`
	Mute        bool   `json:"mute"`
	Volume      map[string]struct {
		ValuePercent string `json:"value_percent"`
	} `json:"volume"`
	Ports      []struct {
		Availability string `json:"availability"`
	} `json:"ports"`
	Properties map[string]string `json:"properties"`
}

// listSinks parses the full sink list with the output-switch filter:
// keep sinks with no ports or any available port. Volume/mute come from
// the same JSON so no per-sink fork is needed.
func (c *Collector) listSinks(ctx context.Context, defName string) ([]SinkEntry, error) {
	out, err := c.run.Run(ctx, "pactl", "-f", "json", "list", "sinks")
	if err != nil {
		return nil, fmt.Errorf("audio: list sinks: %w", err)
	}
	var raw []pactlSink
	if err := json.Unmarshal([]byte(out), &raw); err != nil {
		return nil, fmt.Errorf("audio: list sinks: %w", err)
	}
	sinks := make([]SinkEntry, 0, len(raw))
	for _, s := range raw {
		class, bt := ClassifySink(s.Name, SinkDescription(s), s.Properties)
		sinks = append(sinks, SinkEntry{
			SinkInfo: SinkInfo{
				Name:        s.Name,
				Description: SinkDescription(s),
				Class:       class,
				Bluetooth:   bt,
			},
			Index:     s.Index,
			Available: SinkAvailable(s.Ports),
			IsDefault: s.Name == defName,
			VolumePct: sinkJSONVolume(s.Volume),
			Muted:     s.Mute,
		})
	}
	return sinks, nil
}

// SinkDescription resolves a sink's friendly label with the same fallback
// chain the switch script used: description, device.description, name.
func SinkDescription(s pactlSink) string {
	if s.Description != "" {
		return s.Description
	}
	if v := s.Properties["device.description"]; v != "" {
		return v
	}
	return s.Name
}

// SinkAvailable mirrors the sink-availability filter: no ports, or any
// port not marked "not available".
func SinkAvailable(ports []struct {
	Availability string `json:"availability"`
}) bool {
	if len(ports) == 0 {
		return true
	}
	for _, p := range ports {
		if p.Availability != "not available" {
			return true
		}
	}
	return false
}

// sinkJSONVolume reads the first channel's value_percent ("37%").
func sinkJSONVolume(vol map[string]struct {
	ValuePercent string `json:"value_percent"`
}) int {
	for _, ch := range vol {
		v, err := strconv.Atoi(strings.TrimSuffix(ch.ValuePercent, "%"))
		if err == nil {
			return v
		}
	}
	return 0
}

// pactlSource is one element of `pactl -f json list sources`.
type pactlSource struct {
	Index       int               `json:"index"`
	Name        string            `json:"name"`
	Description string            `json:"description"`
	Properties  map[string]string `json:"properties"`
}

// collectInput fills the source side. It never fails the snapshot: a box
// with no input still reports its output. wpctl is read like input-mute
// reads it (`wpctl get-volume @DEFAULT_AUDIO_SOURCE@`).
func (c *Collector) collectInput(ctx context.Context, run RunFunc, st *Status) {
	st.Sources = []SourceEntry{}
	defName := DefaultSource(ctx, run)
	out, err := c.run.Run(ctx, "pactl", "-f", "json", "list", "sources")
	if err == nil {
		var raw []pactlSource
		if err := json.Unmarshal([]byte(out), &raw); err == nil {
			for _, s := range raw {
				desc := s.Description
				if desc == "" {
					desc = s.Properties["device.description"]
				}
				if desc == "" {
					desc = s.Name
				}
				st.Sources = append(st.Sources, SourceEntry{
					SourceInfo: SourceInfo{
						Name:        s.Name,
						Description: desc,
						Class:       ClassifySource(s.Name, desc, s.Properties),
					},
					Index:     s.Index,
					IsDefault: defName != "" && s.Name == defName,
				})
			}
		}
	}
	for _, s := range st.Sources {
		if s.IsDefault {
			st.Source = s.SourceInfo
		}
	}
	if st.Source.Name == "" && len(st.Sources) > 0 {
		st.Source = st.Sources[0].SourceInfo
	}
	if c.Has("wpctl") {
		if out, err := c.run.Run(ctx, "wpctl", "get-volume", "@DEFAULT_AUDIO_SOURCE@"); err == nil {
			st.InputVolumePct, st.InputMuted = ParseWpctlVolume(out)
		}
	}
}

// ParseWpctlVolume decodes `wpctl get-volume` output ("Volume: 0.80" or
// "Volume: 0.00 [MUTED]"). Pure, for tests and reuse.
func ParseWpctlVolume(out string) (pct int, muted bool) {
	muted = strings.Contains(out, "MUTED")
	for i, tok := range strings.Fields(out) {
		if tok == "Volume:" && i+1 < len(strings.Fields(out)) {
			if v, err := strconv.ParseFloat(strings.Fields(out)[i+1], 64); err == nil {
				return int(v*100 + 0.5), muted
			}
		}
	}
	return 0, muted
}
