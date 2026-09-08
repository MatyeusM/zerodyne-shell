import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.components
import qs.services
import qs.style
import qs.ui

// Performance operate popup: CPU, RAM (with swap), and GPU rows pairing a
// two-line label (the network hero pattern) with a dominant bar carrying
// its own centered value. Fullscreen transparent layer-shell surface;
// scrim click and Escape report closeRequested. Read-only: all state
// arrives via `status performance` + `status gpu` polling, refreshed
// while open.
PanelWindow {
  id: root

  property bool open: false
  property var panelScreen: null
  // Bar x-coordinate to right-align the card to (the indicator's right
  // edge: bar and panel share the screen coordinate space).
  property real anchorX: 0
  // Parsed `zerodyne status performance` / `status gpu` / `status
  // power-profile` payloads.
  property var perf: null
  property var gpu: null
  property var pp: null
  property string error: ""
  readonly property bool connected: root.perf !== null
  readonly property real cpuPct: root.connected ? root.perf.cpu_percent || 0 : 0
  readonly property string cpuName: root.connected && root.perf.cpu_name ? root.perf.cpu_name :
                                                                           "CPU"

  readonly property int cpuCores: root.connected ? root.perf.cpu_cores || 0 : 0
  readonly property real cpuTemp: root.connected && root.perf.cpu_temp_c ? root.perf.cpu_temp_c : 0
  readonly property bool hasTemp: root.connected && root.perf.cpu_temp_c !== undefined
  readonly property real memPct: root.connected ? root.perf.mem_percent || 0 : 0
  readonly property string ppActive: root.pp && root.pp.active ? root.pp.active : ""
  readonly property var ppProfiles: root.pp && root.pp.profiles ? root.pp.profiles : []
  readonly property real cardWidth: Spacing.xxl * 9
  readonly property real labelCol: Spacing.xxl * 2.5

  signal closeRequested

  // Load color trend shared by every bar: primary below 71, secondary to
  // 89, tertiary at 90+.
  function sevColor(p) {
    if (p >= 90)
      return Color.tertiary
    if (p >= 71)
      return Color.secondary
    return Color.primary
  }
  // Overlay label ink over a solid fill: dark on the bright fills, light
  // on tertiary.
  function fillTextColor(p) {
    if (p >= 90)
      return Color.text
    return Color.background
  }
  function singleFill(frac, color) {
    var f = {}
    f.fraction = Math.max(0, Math.min(1, frac))
    f.color = color
    return [f]
  }
  // RAM bar over RAM+swap in one severity color (by memPct): used RAM
  // plus used swap solid, then the still-free swap as a 20% ghost of the
  // same color. Free RAM is track.
  function ramFills() {
    var solid = root.ramSolid()
    if (solid < 0)
      return []
    var out = []
    var used = {}
    used.fraction = solid
    used.color = root.sevColor(root.memPct)
    out.push(used)
    var swapTotal = root.connected ? root.perf.swap_total_kb || 0 : 0
    var swapFree = root.connected ? root.perf.swap_free_kb || 0 : 0
    var memTotal = root.connected ? root.perf.mem_total_kb || 0 : 0
    var total = memTotal + swapTotal
    if (swapTotal > 0 && total > 0) {
      var ghost = {}
      ghost.fraction = swapFree / total
      ghost.color = Color.alpha(root.sevColor(root.memPct), 0.2)
      out.push(ghost)
    }
    return out
  }
  // Solid fraction of the RAM bar: used RAM plus used swap over the
  // combined total. Negative when there is nothing to show.
  function ramSolid() {
    var memTotal = root.connected ? root.perf.mem_total_kb || 0 : 0
    var memAvail = root.connected ? root.perf.mem_avail_kb || 0 : 0
    var swapTotal = root.connected ? root.perf.swap_total_kb || 0 : 0
    var swapFree = root.connected ? root.perf.swap_free_kb || 0 : 0
    var total = memTotal + swapTotal
    if (total <= 0)
      return -1
    return (memTotal - memAvail + swapTotal - swapFree) / total
  }
  function cpuSub() {
    if (!root.connected)
      return "--"
    var parts = []
    if (root.hasTemp)
      parts.push(Math.round(root.cpuTemp) + "\u00b0C")
    if (root.cpuCores > 0)
      parts.push(root.cpuCores + " C")
    if (parts.length === 0)
      return "--"
    return parts.join(" \u00b7 ")
  }
  function ramSub() {
    if (!root.connected)
      return "--"
    var memTotal = root.perf.mem_total_kb || 0
    var swapTotal = root.perf.swap_total_kb || 0
    if (swapTotal > 0)
      return "Swap " + (swapTotal / 1048576).toFixed(1) + " GB \u00b7 " + (memTotal
                                                                           / 1048576).toFixed(1)
          + " GB"
    return (memTotal / 1048576).toFixed(1) + " GB"
  }
  function gpuSub() {
    if (!root.gpu)
      return "--"
    var util = Math.round(root.gpu.util_pct || 0) + "%"
    var temp = Math.round(root.gpu.temp_c || 0) + "\u00b0C"
    var power = Math.round(root.gpu.power_w || 0) + "W"
    return util + " \u00b7 " + temp + " \u00b7 " + power
  }
  function ramText() {
    if (!root.connected)
      return "--"
    var used = (root.perf.mem_total_kb || 0) - (root.perf.mem_avail_kb || 0) + ((root.perf.swap_total_kb
                                                                                 || 0) - (root.perf.swap_free_kb
                                                                                          || 0))
    var total = (root.perf.mem_total_kb || 0) + (root.perf.swap_total_kb || 0)
    return root.gbText(used, total)
  }
  function gbText(usedKb, totalKb) {
    return (usedKb / 1048576).toFixed(1) + " / " + (totalKb / 1048576).toFixed(1) + " GB"
  }
  function refresh() {
    root.error = ""
    if (!perfProc.running)
      perfProc.running = true
    if (!gpuProc.running)
      gpuProc.running = true
    // Power-profile polling is dormant with the selector below.
    // if (!ppProc.running)
    //   ppProc.running = true
  }
  function setProfile(name) {
    if (name === "" || name === root.ppActive || execProc.running)
      return
    execProc.command = Zerodyne.execCommand("power-profile-set", [name])
    execProc.err = ""
    execProc.running = true
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
    id: perfProc

    command: Zerodyne.statusCommand("performance")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        if (result.ok && result.data && result.data.mem_total_kb)
          root.perf = result.data
        else
          root.error = result.error || "performance unavailable"
      }
    }
  }
  Process {
    id: gpuProc

    command: Zerodyne.statusCommand("gpu")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        if (result.ok && result.data && result.data.vram_total_mb)
          root.gpu = result.data
      }
    }
  }
  Process {
    id: ppProc

    command: Zerodyne.statusCommand("power-profile")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        if (result.ok && result.data && result.data.active !== undefined)
          root.pp = result.data
      }
    }
  }
  Process {
    id: execProc

    property string err: ""

    stderr: StdioCollector {
      onStreamFinished: execProc.err = text
    }

    // Block form: only blocks attach to the C++ signal. Re-poll after
    // every set so the selector follows the daemon.
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
    // Below the bar, right edge under the indicator.
    anchors.top: parent.top
    anchors.topMargin: Spacing.lg + Spacing.xs
    x: Math.max(0, root.anchorX - root.cardWidth)
    width: root.cardWidth
    background: Color.surface
    padding: Spacing.md
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferTopLeft: true
    chamferBottomRight: true
    chamferSize: Spacing.sm

    Column {
      width: parent.width
      spacing: Spacing.sm

      PerformanceRow {
        width: parent.width
        title: root.cpuName
        subtitle: root.cpuSub()
        labelWidth: root.labelCol
        fills: root.singleFill(root.cpuPct / 100, root.sevColor(root.cpuPct))
        fillEnd: root.cpuPct / 100
        overlayText: Math.round(root.cpuPct) + "%"
        fillTextColor: root.fillTextColor(root.cpuPct)
      }
      // Power-profile selector, dormant: the backend (`status
      // power-profile` + `power-profile-set`) works, but the UI is
      // undecided — possibly an icon button in front of the CPU row that
      // cycles modes on click. Uncomment to bring the row back.
      /*
      PowerProfileRow {
      visible: root.ppProfiles.length > 0
      width: parent.width
      profiles: root.ppProfiles
      active: root.ppActive

      onSelectProfile: name => root.setProfile(name)
      }
      */
      Separator {
      }
      PerformanceRow {
        width: parent.width
        title: "RAM"
        subtitle: root.ramSub()
        labelWidth: root.labelCol
        fills: root.ramFills()
        fillEnd: Math.max(0, root.ramSolid())
        overlayText: root.ramText()
        fillTextColor: root.fillTextColor(root.memPct)
      }
      Separator {
      }
      PerformanceRow {
        width: parent.width
        title: root.gpu ? root.gpu.name || "GPU" : "GPU"
        subtitle: root.gpuSub()
        labelWidth: root.labelCol
        fills: root.singleFill((root.gpu ? root.gpu.vram_pct || 0 : 0) / 100, root.sevColor(
                                 root.gpu ? root.gpu.vram_pct || 0 : 0))
        fillEnd: (root.gpu ? root.gpu.vram_pct || 0 : 0) / 100
        overlayText: root.gpu ? root.gbText(root.gpu.vram_used_mb * 1024 || 0,
                                            root.gpu.vram_total_mb * 1024 || 0) : "--"
        fillTextColor: root.fillTextColor(root.gpu ? root.gpu.vram_pct || 0 : 0)
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
