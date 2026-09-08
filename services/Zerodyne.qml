pragma Singleton

import Quickshell

// Thin bridge to the zerodyne daemon CLI. All math, parsing, and system
// access live in Go; this only builds argv and decodes the JSON envelope.
//
// Monitor snapshots print the payload itself:
//   {"iface": ..., "type": "wifi", ...}
// Exec results nest under "data" (or print bare "ok"):
//   {"data": {"networks": [...]}} / ok
Singleton {
  readonly property string bin: Quickshell.shellPath("bin/zerodyne")

  // Nerd-font link glyphs (rendered with the mono font). Codepoints
  // verified against the shipped JetBrainsMono Nerd Font.
  readonly property string glyphWifiOff: "\uDB82\uDD2E"
  readonly property string glyphEthernet: "\uDB80\uDE00"
  readonly property string glyphQr: "\uDB81\uDC32"
  readonly property string glyphForget: "\uDB81\uDED1"
  readonly property string glyphLock: "\uDB80\uDF3E"
  // Material bluetooth set, verified against the shipped JetBrainsMono
  // Nerd Font with fontTools. Codepoints are astral (U+F00AF+), so they
  // must be surrogate pairs: QML \u takes exactly 4 hex digits and
  // "\uf00af" would parse as U+F00A plus a literal "f".
  readonly property string glyphBluetooth: "\udb80\udcaf"
  readonly property string glyphBluetoothOff: "\udb80\udcb2"
  readonly property string glyphBluetoothConnected: "\udb80\udcb1"
  readonly property var glyphWifiLevels: ["\uDB82\uDD2F", "\uDB82\uDD1F", "\uDB82\uDD22",
    "\uDB82\uDD25", "\uDB82\uDD28"]

  // BlueZ device-class glyphs (MD bluetooth set, astral: surrogate pairs).
  readonly property string glyphBtHeadphones: "\udb82\udd70"
  readonly property string glyphBtPhone: "\udb80\udff3"
  readonly property string glyphBtMouse: "\udb82\udd8b"
  readonly property string glyphBtSpeakers: "\udb82\udda2"

  // Audio glyphs. Art sources: the waybar pulseaudio contract
  // (~/.config/waybar/config.jsonc: format-icons, format-muted,
  // format-bluetooth) and omarchy's audio Panel outputIcon/inputIcon plus
  // Model sinkGlyph/sourceGlyph. All codepoints verified against the
  // shipped JetBrainsMono Nerd Font with fontTools; astral (U+F0000+)
  // codepoints are surrogate pairs (QML \u takes exactly 4 hex digits).
  // The bluetooth badge reuses glyphBluetooth (same art as waybar's
  // format-bluetooth marker). Mute art is omarchy's outputIcon muted
  // branch; the ladder uses waybar's default[0..2] at omarchy's
  // >=67/>=34/>0 thresholds; mic art is omarchy's Microphone widget.
  readonly property string glyphAudioMuted: "\ueee8"
  readonly property string glyphAudioLow: "\udb81\udd7f"
  readonly property string glyphAudioMed: "\udb81\udd80"
  readonly property string glyphAudioHigh: "\udb81\udd7e"
  readonly property string glyphAudioHeadphones: "\udb80\udecb"
  readonly property string glyphAudioHeadset: "\udb80\udece"
  readonly property string glyphAudioPhone: "\udb80\udff2"
  readonly property string glyphAudioHdmi: "\udb80\udf79"
  readonly property string glyphAudioCar: "\udb80\udd0b"
  readonly property string glyphAudioMic: "\udb80\udf6c"
  readonly property string glyphAudioMicMuted: "\udb80\udf6d"
  readonly property string glyphAudioCamera: "\udb80\udd00"
  // Power menu glyphs (lock reuses glyphLock). Codepoints verified against
  // the shipped JetBrainsMono Nerd Font with fontTools; astral codepoints
  // are surrogate pairs.
  readonly property string glyphPowerLogout: "\udb80\udc04"
  readonly property string glyphPowerShutdown: "\udb81\udc25"
  readonly property string glyphPowerRestart: "\udb81\udc67"
  // Performance battery glyphs: the indicator borrows the battery
  // art for load levels (10 / 90 / full). Codepoints verified against
  // the shipped JetBrainsMono Nerd Font with fontTools.
  readonly property string glyphBatt10: "\udb80\udc7a"
  readonly property string glyphBatt20: "\udb80\udc7b"
  readonly property string glyphBatt30: "\udb80\udc7c"
  readonly property string glyphBatt40: "\udb80\udc7d"
  readonly property string glyphBatt50: "\udb80\udc7e"
  readonly property string glyphBatt60: "\udb80\udc7f"
  readonly property string glyphBatt70: "\udb80\udc80"
  readonly property string glyphBatt80: "\udb80\udc81"
  readonly property string glyphBatt90: "\udb80\udc82"
  readonly property string glyphBattFull: "\udb80\udc79"

  function statusCommand(target) {
    return [bin, "status", target]
  }
  function execCommand(operation, args) {
    return [bin, "exec", operation].concat(args || [])
  }
  function parseOutput(text) {
    var trimmed = String(text || "").trim()
    if (trimmed === "")
      return {
        "ok": false,
        "error": "empty response"
      }
    if (trimmed === "ok")
      return {
        "ok": true,
        "data": null
      }
    try {
      var parsed = JSON.parse(trimmed)
      if (parsed && typeof parsed.data === "object" && parsed.data !== null)
        return {
          "ok": true,
          "data": parsed.data
        }
      return {
        "ok": true,
        "data": parsed
      }
    } catch (e) {
      return {
        "ok": false,
        "error": "bad response"
      }
    }
  }

  // Presentation mapping from link state to glyph. Thresholds mirror the
  // old panel: five levels across 0-100.
  function linkGlyph(type, signalPct) {
    if (type === "ethernet")
      return glyphEthernet
    if (type !== "wifi")
      return glyphWifiOff
    var index = Math.max(0, Math.min(4, Math.ceil((signalPct || 0) / 20) - 1))
    return glyphWifiLevels[index]
  }

  // Presentation mapping from bluetooth state to glyph.
  function bluetoothGlyph(powered, connected) {
    if (!powered)
      return glyphBluetoothOff
    if (connected)
      return glyphBluetoothConnected
    return glyphBluetooth
  }

  // Device type from the BlueZ icon name (audio-headset, input-mouse,
  // ...). Substring match in specificity order: "headphone" contains
  // "phone", so it must come first. Falls back to the state glyph.
  function bluetoothDeviceGlyph(icon, connected) {
    var name = String(icon || "").toLowerCase()
    if (name.indexOf("headset") >= 0 || name.indexOf("headphone") >= 0)
      return glyphBtHeadphones
    if (name.indexOf("mouse") >= 0)
      return glyphBtMouse
    if (name.indexOf("speaker") >= 0)
      return glyphBtSpeakers
    if (name.indexOf("phone") >= 0)
      return glyphBtPhone
    if (connected)
      return glyphBluetoothConnected
    return glyphBluetooth
  }

  // Presentation mapping from the Go audio class to a device glyph.
  // Thresholds mirror omarchy Panel outputIcon (>=67/>=34/>0); muted
  // short-circuits. Inputs are the Go class plus numbers only.
  function audioDeviceGlyph(cls, volumePct, muted) {
    if (muted)
      return glyphAudioMuted
    if (cls === "headphones")
      return glyphAudioHeadphones
    if (cls === "headset")
      return glyphAudioHeadset
    if (cls === "phone")
      return glyphAudioPhone
    if (cls === "hdmi")
      return glyphAudioHdmi
    if (cls === "car")
      return glyphAudioCar
    var v = volumePct || 0
    if (v >= 67)
      return glyphAudioHigh
    if (v >= 34)
      return glyphAudioMed
    if (v > 0)
      return glyphAudioLow
    return glyphAudioMuted
  }

  // Presentation mapping from input mute state to the mic glyph
  // (omarchy Microphone widget art).
  function audioSourceGlyph(inputMuted) {
    return inputMuted ? glyphAudioMicMuted : glyphAudioMic
  }

  // Presentation mapping from a load percentage to the battery art.
  function perfBatteryGlyph(pct) {
    var v = pct || 0
    if (v >= 100)
      return glyphBattFull
    if (v >= 90)
      return glyphBatt90
    if (v >= 80)
      return glyphBatt80
    if (v >= 70)
      return glyphBatt70
    if (v >= 60)
      return glyphBatt60
    if (v >= 50)
      return glyphBatt50
    if (v >= 40)
      return glyphBatt40
    if (v >= 30)
      return glyphBatt30
    if (v >= 20)
      return glyphBatt20
    return glyphBatt10
  }

  // Presentation mapping from a Go source class to its row glyph.
  function audioSourceDeviceGlyph(cls) {
    if (cls === "headset")
      return glyphAudioHeadset
    if (cls === "camera")
      return glyphAudioCamera
    if (cls === "phone")
      return glyphAudioPhone
    return glyphAudioMic
  }

  // CLI failures print "<op>: <detail>" on stderr; show only the detail.
  function errorText(stderr) {
    var msg = String(stderr || "").trim()
    if (msg === "")
      return "command failed"
    var i = msg.indexOf(": ")
    return i >= 0 ? msg.slice(i + 2).trim() : msg
  }
}
