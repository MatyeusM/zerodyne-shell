import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.components
import qs.services
import qs.style
import qs.ui

// Audio operate popup: OUTPUT section (slider + sink rows) and INPUT
// section (slider + source rows). Floating-card pattern like NetworkPanel
// (fullscreen transparent layer-shell, scrim + Escape close); the card
// left-aligns to the indicator's screen x. All system access goes through
// zerodyne; this file only renders and forwards actions.
//
// Adapted from omarchy's audio Panel (section layout, muted dimming,
// right-click slider mute); no per-app streams, no peak meter, no cursor
// model, no mood ladder — same consistency cut as our other panels.
PanelWindow {
  id: root

  property bool open: false
  property var panelScreen: null
  // Bar x-coordinate to left-align the card to (the indicator's left
  // edge: bar and panel share the screen coordinate space).
  property real anchorX: 0
  // Parsed `zerodyne status audio` payload, or null when broken.
  property var audio: null
  property string error: ""
  readonly property bool connected: root.audio !== null
  readonly property int volumePct: root.connected ? root.audio.volume_pct || 0 : 0
  readonly property bool outMuted: !root.connected || root.audio.muted === true
  readonly property string sinkDesc: root.connected && root.audio.sink ? root.audio.sink.description
                                                                         || "" : ""
  readonly property var sinks: root.connected && root.audio.sinks ? root.audio.sinks : []
  readonly property int inputVolumePct: root.connected ? root.audio.input_volume_pct || 0 : 0
  readonly property bool inputMuted: !root.connected || root.audio.input_muted !== false
  readonly property var sources: root.connected && root.audio.sources ? root.audio.sources : []

  signal closeRequested

  function refresh() {
    root.error = ""
    if (!statusProc.running)
      statusProc.running = true
  }
  function runExec(op, args) {
    if (execProc.running)
      return
    execProc.command = Zerodyne.execCommand(op, args)
    execProc.out = ""
    execProc.err = ""
    execProc.running = true
  }
  function setOutputVolume(v) {
    var delta = Math.round(v) - root.volumePct
    if (delta === 0)
      return
    root.runExec("audio-volume", [(delta > 0 ? "+" : "") + delta])
  }
  function toggleOutputMute() {
    root.runExec("audio-volume", ["mute-toggle"])
  }
  function setInputVolume(v) {
    var delta = Math.round(v) - root.inputVolumePct
    if (delta === 0)
      return
    root.runExec("audio-input-mute", [(delta > 0 ? "+" : "") + delta])
  }
  function toggleInputMute() {
    root.runExec("audio-input-mute", ["toggle"])
  }
  function setDefaultSink(index, name) {
    root.runExec("audio-output-default", [String(index), name])
  }
  function setDefaultSource(index, name) {
    root.runExec("audio-input-default", [String(index), name])
  }

  visible: root.open
  anchors.top: true
  anchors.bottom: true
  anchors.left: true
  anchors.right: true
  color: "transparent"
  exclusionMode: ExclusionMode.Ignore
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
  screen: root.panelScreen

  onOpenChanged: {
    if (root.open)
      root.refresh()
  }

  Timer {
    interval: 3000
    running: root.open
    repeat: true

    onTriggered: root.refresh()
  }
  Process {
    id: statusProc

    command: Zerodyne.statusCommand("audio")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        if (result.ok && result.data && result.data.volume_sink !== undefined)
          root.audio = result.data
        else
          root.error = result.error || "audio unavailable"
      }
    }
  }
  Process {
    id: execProc

    property string out: ""
    property string err: ""

    stdout: StdioCollector {
      onStreamFinished: execProc.out = text
    }
    stderr: StdioCollector {
      onStreamFinished: execProc.err = text
    }

    // Block form (not `function onExited`): only blocks attach to the C++
    // signal. Every exec re-polls; scroll/slider feedback depends on it.
    onExited: {
      if (execProc.err !== "")
        root.error = Zerodyne.errorText(execProc.err)
      root.refresh()
    }
  }
  MouseArea {
    anchors.fill: parent

    onClicked: root.closeRequested()
  }
  Item {
    anchors.fill: parent
    focus: root.open

    Keys.onEscapePressed: root.closeRequested()
  }
  Panel {
    // Below the bar, left edge under the indicator.
    anchors.top: parent.top
    anchors.topMargin: Spacing.lg + Spacing.xs
    x: Math.max(0, root.anchorX)
    width: Spacing.xxl * 6
    background: Color.surface
    padding: Spacing.md
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferTopRight: true
    chamferBottomLeft: true
    chamferSize: Spacing.sm

    Column {
      width: parent.width
      spacing: Spacing.sm

      RowLayout {
        width: parent.width
        spacing: Spacing.sm

        Text {
          Layout.fillWidth: true
          elide: Text.ElideRight
          text: root.sinkDesc !== "" ? root.sinkDesc : "Audio"
          color: Color.text
          font: Typography.display()
        }
        Text {
          text: root.volumePct + "%"
          color: root.outMuted ? Color.muted : Color.text
          font: Typography.mono()
          opacity: root.outMuted ? 0.5 : 1.0
        }
      }
      Text {
        text: "OUTPUT"
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
      AudioSlider {
        width: parent.width
        value: root.volumePct
        opacity: root.outMuted ? 0.5 : 1.0

        onMoved: v => root.setOutputVolume(v)
        onMuteRequested: root.toggleOutputMute()
      }
      ListView {
        width: parent.width
        height: Math.min(contentHeight, Spacing.xxl * 3)
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.sinks

        ScrollBar.vertical: AutoScrollbar {
          id: sinkBar
        }
        delegate: AudioDeviceRow {
          required property var modelData

          width: ListView.view.width - (ListView.view.contentHeight > ListView.view.height
                                        ? sinkBar.gutter : 0)
          glyph: Zerodyne.audioDeviceGlyph(modelData.class, modelData.volume_pct, modelData.muted)
          label: modelData.description
          active: modelData.is_default

          onActivated: root.setDefaultSink(modelData.index, modelData.name)
        }
      }
      Text {
        visible: root.sinks.length === 0
        text: "No outputs found"
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
      Separator {
      }
      Text {
        text: "INPUT"
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
      AudioSlider {
        width: parent.width
        value: root.inputVolumePct
        opacity: root.inputMuted ? 0.5 : 1.0

        onMoved: v => root.setInputVolume(v)
        onMuteRequested: root.toggleInputMute()
      }
      ListView {
        width: parent.width
        height: Math.min(contentHeight, Spacing.xxl * 3)
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.sources

        ScrollBar.vertical: AutoScrollbar {
          id: sourceBar
        }
        delegate: AudioDeviceRow {
          required property var modelData

          width: ListView.view.width - (ListView.view.contentHeight > ListView.view.height
                                        ? sourceBar.gutter : 0)
          glyph: Zerodyne.audioSourceDeviceGlyph(modelData.class)
          label: modelData.description
          active: modelData.is_default

          onActivated: root.setDefaultSource(modelData.index, modelData.name)
        }
      }
      Text {
        visible: root.sources.length === 0
        text: "No inputs found"
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
      Text {
        visible: root.error !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: root.error
        color: Color.tertiary
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
    }
  }
}
