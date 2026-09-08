import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.components
import qs.services
import qs.style
import qs.ui

// Network operate popup: connection hero, visible wifi list, band control.
// Fullscreen transparent layer-shell surface (compositor never sizes the
// card); scrim click and Escape report closeRequested. All system access
// goes through zerodyne; this file only renders and forwards actions.
PanelWindow {
  id: root

  property bool open: false
  property var panelScreen: null
  // Bar x-coordinate to right-align the card to (the indicator's right
  // edge: bar and panel share the screen coordinate space).
  property real anchorX: 0
  property var link: null
  property var networks: []
  property bool scanning: false
  property bool busy: false
  property string error: ""
  property string promptFor: ""
  property var band: null
  // Set after a confirmed disconnect: the next cached scan read still
  // carries stale in-use flags, so strip them instead of letting the
  // cache re-disable the rows before the forced rescan returns.
  property bool dropStaleUse: false

  signal closeRequested
  signal showQr

  function refresh() {
    root.error = ""
    root.promptFor = ""
    fetchStatus()
    fetchScan()
    fetchBand()
  }
  function fetchStatus() {
    if (!statusProc.running)
      statusProc.running = true
  }
  // Instant: the wifi monitor rescans lazily in the background.
  function fetchScan() {
    if (scanProc.running)
      return
    scanProc.command = Zerodyne.statusCommand("wifi")
    scanProc.running = true
  }
  // Forced synchronous rescan after state changes: `status wifi` serves a
  // background cache (up to 30s stale), so without this the list keeps the
  // old in-use flags and rows stay unclickable after disconnect/connect.
  function refreshScan() {
    if (!rescanProc.running)
      rescanProc.running = true
  }
  // Instant optimistic update after a confirmed disconnect: clear stale
  // in-use flags so rows are clickable before the rescan returns.
  function clearInUse() {
    root.networks = root.networks.map(function (net) {
      net.in_use = false
      return net
    })
  }
  function fetchBand() {
    runExec("wifi-band", [], function (ok, out, err) {
      if (ok)
        root.band = Zerodyne.parseOutput(out).data
    })
  }
  function runExec(op, args, done) {
    if (execProc.running)
      return
    execProc.command = Zerodyne.execCommand(op, args)
    execProc.done = done
    execProc.out = ""
    execProc.err = ""
    execProc.running = true
  }
  function connect(ssid, password) {
    root.busy = true
    var args = password !== "" ? [ssid, password] : [ssid]
    runExec("wifi-connect", args, function (ok, out, err) {
      root.busy = false
      if (!ok) {
        var msg = Zerodyne.errorText(err)
        if (msg === "need-password") {
          root.promptFor = ssid
          root.error = "Password required for " + ssid
        } else {
          root.error = msg
        }
        return
      }
      root.promptFor = ""
      root.refresh()
      root.refreshScan()
    })
  }
  function activateRow(net) {
    if (net.in_use)
      return
    if (net.security !== "open" && !net.known && root.promptFor !== net.ssid) {
      root.promptFor = net.ssid
      return
    }
    root.connect(net.ssid, "")
  }
  function setBand(value) {
    runExec("wifi-band", [value], function (ok, out, err) {
      if (!ok)
        root.error = Zerodyne.errorText(err)
      root.fetchBand()
      root.fetchStatus()
    })
  }
  function disconnect() {
    runExec("wifi-disconnect", [], function (ok, out, err) {
      if (!ok) {
        root.error = Zerodyne.errorText(err)
        root.refresh()
        return
      }
      root.clearInUse()
      root.dropStaleUse = true
      root.refresh()
      root.refreshScan()
    })
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
  Component.onCompleted: {
    if (root.open)
      root.refresh()
  }

  // Live updates while open: stats every few seconds, networks as the
  // background monitor rescans. No buttons; the daemon owns freshness.
  Timer {
    interval: 3000
    running: root.open
    repeat: true

    onTriggered: root.fetchStatus()
  }
  Timer {
    interval: 15000
    running: root.open
    repeat: true

    onTriggered: root.fetchScan()
  }
  Process {
    id: statusProc

    command: Zerodyne.statusCommand("network")

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        root.link = result.ok && result.data && result.data.type ? result.data : null
      }
    }
  }
  Process {
    id: scanProc

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        var nets = result.ok && result.data ? result.data.networks || [] : []
        if (root.dropStaleUse) {
          root.dropStaleUse = false
          nets = nets.map(function (net) {
            net.in_use = false
            return net
          })
        }
        root.networks = nets
        if (!result.ok)
          root.error = result.error
      }
    }

    onStarted: root.scanning = true
    onExited: root.scanning = false
  }
  // Forced rescan (`exec wifi-scan rescan` returns fresh networks under
  // data): reconciles the list right after disconnect/connect/forget
  // instead of waiting for the 30s background refresh. Own Process so ops
  // on execProc are never blocked behind a seconds-long radio scan.
  Process {
    id: rescanProc

    command: Zerodyne.execCommand("wifi-scan", ["rescan"])

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        if (result.ok && result.data)
          root.networks = result.data.networks || []
        else
          root.fetchScan()
      }
    }

    onStarted: root.scanning = true
    onExited: {
      root.scanning = false
    }
  }
  Process {
    id: execProc

    property var done: null
    property string out: ""
    property string err: ""

    stdout: StdioCollector {
      onStreamFinished: execProc.out = text
    }
    stderr: StdioCollector {
      onStreamFinished: execProc.err = text
    }

    // Block form (not `function onExited`): only blocks attach to the C++
    // signal. Success is read from stderr instead of the injected exit
    // code, which Qt 6 deprecated.
    onExited: {
      var done = execProc.done
      execProc.done = null
      if (done)
        done(execProc.err === "", execProc.out, execProc.err)
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
    x: Math.max(0, root.anchorX - implicitWidth)
    background: Color.surface
    padding: Spacing.md
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferTopLeft: true
    chamferBottomRight: true
    chamferSize: Spacing.sm

    Column {
      spacing: Spacing.sm

      ConnectionHero {
        id: hero

        width: parent.width
        link: root.link

        onShowQr: root.showQr()
        onDisconnectRequested: root.disconnect()
      }
      Text {
        visible: root.error !== ""
        width: hero.width
        wrapMode: Text.Wrap
        text: root.error
        color: Color.tertiary
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
      Separator {
        visible: root.link !== null
      }
      LinkStats {
        link: root.link
      }
      Separator {
        visible: root.link !== null && root.band !== null
      }
      BandSelector {
        width: parent.width
        link: root.link
        band: root.band

        onSelectBand: value => root.setBand(value)
      }
      Separator {
      }
      WifiList {
        width: parent.width
        networks: root.networks
        promptFor: root.promptFor
        busy: root.busy
        scanning: root.scanning

        onConnectRequested: (ssid, password) => root.connect(ssid, password)
        onCancelPrompt: root.promptFor = ""
        onForgetRequested: ssid => root.runExec("wifi-forget", [ssid], function (ok, out, err) {
          if (!ok)
            root.error = Zerodyne.errorText(err)
          root.refresh()
          root.refreshScan()
        })
      }
    }
  }
}
