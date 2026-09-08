import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.components
import qs.services
import qs.style
import qs.ui

// Bar performance indicator: top load percent plus a battery glyph for
// the level. The reading is the max of CPU, memory, and VRAM utilization;
// color follows the load trend (70%+ secondary, 90%+ tertiary).
// Full bar height, tucked under the tray so both bottom borders join.
//
// Battery-discharge awareness is prepared but not executed: `discharging`
// will drive an alternate color scheme once battery state exists; it
// stays false until then.
Item {
  id: root

  property var panelScreen: null
  property bool perfOpen: false
  // Parsed `zerodyne status performance` / `status gpu` payloads.
  property var perf: null
  property var gpu: null
  // Reserved for the future battery backend; always false for now.
  property bool discharging: false
  readonly property bool connected: root.perf !== null
  readonly property real cpuPct: root.connected ? root.perf.cpu_percent || 0 : 0
  readonly property real memPct: root.connected ? root.perf.mem_percent || 0 : 0
  readonly property real vramPct: root.gpu ? root.gpu.vram_pct || 0 : 0
  readonly property real topPct: Math.max(root.cpuPct, root.memPct, root.vramPct)
  // Inset past the tray overlap (its chamfer cut) plus breathing room.
  readonly property real rowLeft: Spacing.sm + Spacing.sm
  readonly property real rowRight: Spacing.sm + Spacing.sm
  readonly property real iconSize: Typography.lg

  function topSource() {
    if (root.vramPct >= root.memPct && root.vramPct >= root.cpuPct)
      return "VRAM"
    if (root.memPct >= root.cpuPct)
      return "RAM"
    return "CPU"
  }
  function loadColor() {
    if (!root.connected)
      return Color.muted
    if (root.topPct >= 90)
      return Color.tertiary
    if (root.topPct >= 71)
      return Color.secondary
    return Color.primary
  }
  function refresh() {
    if (!perfProc.running)
      perfProc.running = true
    if (!gpuProc.running)
      gpuProc.running = true
  }

  implicitWidth: root.rowLeft + root.iconSize + root.rowRight
  implicitHeight: Spacing.lg

  Component.onCompleted: root.refresh()

  Timer {
    interval: 5000
    running: true
    repeat: true

    onTriggered: root.refresh()
  }
  Process {
    id: perfProc

    command: Zerodyne.statusCommand("performance")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        root.perf = result.ok && result.data && result.data.mem_total_kb ? result.data : null
      }
    }
  }
  Process {
    id: gpuProc

    command: Zerodyne.statusCommand("gpu")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        root.gpu = result.ok && result.data && result.data.vram_total_mb ? result.data : null
      }
    }
  }
  Clickable {
    id: click

    anchors.fill: parent

    onClicked: root.perfOpen = !root.perfOpen
  }
  HoverColor {
    id: hoverFg

    hovered: click.hovered
    normalColor: root.loadColor()
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
      spacing: Spacing.xs

      Icon {
        id: battIcon

        Layout.alignment: Qt.AlignVCenter
        glyph: Zerodyne.perfBatteryGlyph(root.topPct)
        size: root.iconSize
        color: root.connected ? hoverFg.current : Color.muted
      }
    }
  }
  Tooltip {
    anchor.item: battIcon
    anchor.edges: Qt.BottomEdge
    anchor.gravity: Qt.BottomEdge
    text: root.topSource()
    visible: click.hovered && root.connected
  }
  Border {
    anchors.fill: parent
    showTop: false
    // No right edge: tucked under the tray, whose left edge divides.
    showRight: false
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferBottomLeft: true
    chamferSize: Spacing.sm
  }
  PerformancePanel {
    open: root.perfOpen
    panelScreen: root.panelScreen
    // Bar and panel share screen coordinates; align the card's right
    // edge with the indicator's.
    anchorX: root.x + root.width

    onCloseRequested: root.perfOpen = false
  }
}
