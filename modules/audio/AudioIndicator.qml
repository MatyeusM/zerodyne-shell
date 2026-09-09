import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.components
import qs.services
import qs.style
import qs.ui

// Bar audio indicator: output percent, device-class icon, bluetooth badge,
// mic icon. Scroll on the output zone adjusts output volume, scroll on the
// mic zone adjusts input volume, middle toggles output mute, right (or mic
// click) toggles input mute, left opens the panel.
// Layout mirrors ConnectivityIndicator: same height, insets, border and
// chamfer rhythm, tucked under the clock on the left instead of the right.
//
// Adapted from omarchy's audio Panel (outputIcon/inputIcon, wheel
// accumulator) and the waybar pulseaudio contract; all state arrives via
// `status audio` polling, never Pipewire in QML.
Item {
  id: root

  property var panelScreen: null
  property bool audioOpen: false
  // Parsed `zerodyne status audio` payload, or null when broken.
  property var audio: null
  readonly property bool connected: root.audio !== null
  readonly property int volumePct: root.connected ? root.audio.volume_pct || 0 : 0
  readonly property bool outMuted: !root.connected || root.audio.muted === true
  readonly property string sinkClass: root.connected && root.audio.sink ? root.audio.sink.class
                                                                          || "other" : "other"
  readonly property bool sinkBt: root.connected && root.audio.sink ? root.audio.sink.bluetooth
                                                                     === true : false
  readonly property bool sourcePresent: root.connected && root.audio.source ? !
                                                                              !root.audio.source.name :
                                                                              false
  readonly property bool inputMuted: !root.connected || root.audio.input_muted !== false
  // Inset past the clock overlap (its chamfer cut) plus breathing room.
  readonly property real rowLeft: Spacing.sm + Spacing.sm
  readonly property real rowRight: Spacing.sm + Spacing.sm
  readonly property real iconSize: Typography.lg
  // Carries sub-notch touchpad deltas between wheel events (omarchy
  // Panel wheelAccumulator pattern: small UI-side input smoothing). One
  // accumulator per wheel zone so output and mic remainders never mix.
  property real wheelAcc: 0
  property real micWheelAcc: 0

  function percentText() {
    return root.connected ? root.volumePct + "%" : "--"
  }
  function deviceGlyph() {
    if (!root.connected)
      return Zerodyne.glyphAudioMuted
    return Zerodyne.audioDeviceGlyph(root.sinkClass, root.volumePct, root.outMuted)
  }
  function refresh() {
    if (!statusProc.running)
      statusProc.running = true
  }
  function runExec(op, args) {
    if (execProc.running)
      return
    execProc.command = Zerodyne.execCommand(op, args)
    execProc.running = true
  }
  function toggleOutputMute() {
    root.runExec("audio-volume", ["mute-toggle"])
  }
  function toggleInputMute() {
    root.runExec("audio-input-mute", ["toggle"])
  }
  function adjustVolume(steps) {
    if (steps === 0)
      return
    var delta = steps * 5
    root.runExec("audio-volume", [(delta > 0 ? "+" : "") + delta])
  }
  function adjustInputVolume(steps) {
    if (steps === 0)
      return
    var delta = steps * 5
    root.runExec("audio-input-mute", [(delta > 0 ? "+" : "") + delta])
  }

  implicitWidth: Sizing.barIndicatorWidth
  implicitHeight: Spacing.lg

  Component.onCompleted: root.refresh()

  // Fixed width shared with ConnectivityIndicator (Sizing.barIndicatorWidth)
  // so both clock arms match. The percent Text takes its natural width so
  // the group breathes when digits come and go, and the group centers
  // between fill spacers so it always reads as centered.
  Timer {
    interval: 5000
    running: true
    repeat: true

    onTriggered: root.refresh()
  }
  Process {
    id: statusProc

    command: Zerodyne.statusCommand("audio")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        root.audio = result.ok && result.data && result.data.volume_sink !== undefined
            ? result.data : null
      }
    }
  }
  // Fire-and-forget toggles and volume steps: the indicator follows on
  // the immediate re-poll (scroll feedback depends on it).
  Process {
    id: execProc

    stdout: StdioCollector {
    }

    onExited: {
      root.refresh()
    }
  }
  Clickable {
    id: micClick

    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.right: parent.right
    width: root.rowRight + root.iconSize
    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton

    onClicked: mouse => {
      if (mouse.button === Qt.MiddleButton)
        root.toggleOutputMute()
      else
        root.toggleInputMute()
    }
  }
  Clickable {
    id: outClick

    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: micClick.left
    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton

    onClicked: mouse => {
      if (mouse.button === Qt.MiddleButton) {
        root.toggleOutputMute()
        return
      }
      if (mouse.button === Qt.RightButton) {
        root.toggleInputMute()
        return
      }
      root.audioOpen = !root.audioOpen
    }
  }
  HoverColor {
    id: outHover

    hovered: outClick.hovered
    normalColor: Color.text
    hoverColor: Color.secondary
  }
  HoverColor {
    id: micHover

    hovered: micClick.hovered
    normalColor: Color.text
    hoverColor: Color.secondary
  }
  Container {
    anchors.fill: parent
    background: Color.surface
    cornerStyle: Container.Chamfered
    chamferBottomLeft: true
    chamferSize: Spacing.sm

    RowLayout {
      anchors.fill: parent
      anchors.leftMargin: root.rowLeft
      anchors.rightMargin: root.rowRight
      spacing: 0

      Item {
        Layout.fillWidth: true
      }
      RowLayout {
        Layout.alignment: Qt.AlignVCenter
        spacing: Spacing.xs

        Text {
          Layout.alignment: Qt.AlignVCenter
          text: root.percentText()
          color: root.outMuted ? Color.muted : outHover.current
          font: Typography.mono({
                                  "size": Typography.sm
                                })
        }
        // Device icon plus badge as one visual unit: the badge tucks into
        // its icon with a negative gap so the pair reads as one glyph.
        Row {
          Layout.alignment: Qt.AlignVCenter
          spacing: -Spacing.xxs

          Icon {
            glyph: root.deviceGlyph()
            size: root.iconSize
            color: root.outMuted ? Color.muted : outHover.current
          }
          Icon {
            visible: root.sinkBt
            glyph: Zerodyne.glyphBluetooth
            size: root.iconSize
            color: root.outMuted ? Color.muted : outHover.current
          }
        }
        Icon {
          Layout.alignment: Qt.AlignVCenter
          glyph: Zerodyne.audioSourceGlyph(root.inputMuted)
          size: root.iconSize
          color: !root.sourcePresent || root.inputMuted ? Color.muted : micHover.current
        }
      }
      Item {
        Layout.fillWidth: true
      }
    }
  }
  Border {
    anchors.fill: parent
    showTop: false
    // No right edge: sits flush against the clock, whose left edge divides.
    showRight: false
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferBottomLeft: true
    chamferSize: Spacing.sm
  }
  // Wheel overlays above the click zones, one per zone: NoButton accepts
  // nothing, so clicks fall through to the Clickables below while wheel
  // events land here. Output zone drives output volume, mic zone drives
  // input volume. Clickable stays primitive (no wheel support there).
  MouseArea {
    id: micWheel

    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.right: parent.right
    width: root.rowRight + root.iconSize
    acceptedButtons: Qt.NoButton

    onWheel: event => {
      root.micWheelAcc += event.angleDelta.y
      var steps = Math.trunc(root.micWheelAcc / 120)
      root.micWheelAcc -= steps * 120
      root.adjustInputVolume(steps)
    }
  }
  MouseArea {
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: micWheel.left
    acceptedButtons: Qt.NoButton

    onWheel: event => {
      root.wheelAcc += event.angleDelta.y
      var steps = Math.trunc(root.wheelAcc / 120)
      root.wheelAcc -= steps * 120
      root.adjustVolume(steps)
    }
  }
  AudioPanel {
    open: root.audioOpen
    panelScreen: root.panelScreen
    // Bar and panel share screen coordinates; align the card's left
    // edge with the indicator's.
    anchorX: root.x

    onCloseRequested: root.audioOpen = false
  }
}
