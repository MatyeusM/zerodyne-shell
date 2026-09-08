import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Io
import Quickshell.Wayland
import qs.components
import qs.services
import qs.style
import qs.ui

// Bluetooth operate popup: hero with power action plus the device list.
// Fullscreen transparent layer-shell surface (the compositor never sizes
// the card); scrim click and Escape report closeRequested.
//
// Monitoring is live via Quickshell.Bluetooth; all state changes go
// through zerodyne (`bluetooth-power`, `bluetooth-device`). Device
// grouping and rows live in BluetoothDeviceList; this file owns the
// window, discovery, pending actions, and zerodyne calls.
PanelWindow {
  id: root

  property bool open: false
  property var panelScreen: null
  // Bar x-coordinate to right-align the card to (the indicator's right
  // edge: bar and panel share the screen coordinate space).
  property real anchorX: 0
  // Address -> "connecting" | "disconnecting" | "forgetting". Keeps rows
  // responsive while BlueZ catches up; reconciled against live state.
  property var pendingActions: ({})
  // True while this instance owes BlueZ a StopDiscovery: set when it
  // starts discovery (or opens onto a running session) and cleared once
  // discovery is confirmed down after close. Without this a visit leaves
  // the radio in inquiry and starves A2DP audio into stutters.
  property bool owesDiscoveryStop: false
  property bool busy: false
  property string error: ""
  readonly property var adapter: Bluetooth.defaultAdapter
  readonly property bool available: root.adapter !== null
  readonly property bool powered: root.available && root.adapter.enabled
  readonly property bool discovering: root.powered && root.adapter.discovering
  readonly property var rawDevices: Bluetooth.devices ? Bluetooth.devices.values : []
  readonly property int connectedCount: root.countConnected(root.rawDevices)
  readonly property string emptyText: !root.available ? "No Bluetooth adapter" : !root.powered
                                                        ? "Turn Bluetooth on to scan" :
                                                          root.discovering
                                                          ? "Scanning for devices…" :
                                                            "No devices found"

  signal closeRequested

  function setPendingAction(address, action) {
    if (address === "")
      return
    var next = {}
    for (var key in root.pendingActions)
      next[key] = root.pendingActions[key]
    if (action)
      next[address] = action
    else
      delete next[address]
    root.pendingActions = next
    if (action)
      pendingTimeout.restart()
  }
  function runExec(op, args, done) {
    if (execProc.running)
      return
    root.busy = true
    execProc.command = Zerodyne.execCommand(op, args)
    execProc.done = done
    execProc.out = ""
    execProc.err = ""
    execProc.running = true
  }
  function runDeviceAction(address, action, pending) {
    if (address === "")
      return
    root.error = ""
    root.setPendingAction(address, pending)
    runExec("bluetooth-device", [action, address], function (ok, out, err) {
      if (!ok)
        root.error = Zerodyne.errorText(err)
    })
  }
  function connectDevice(address) {
    root.runDeviceAction(address, "connect", "connecting")
  }
  function pairDevice(address) {
    root.runDeviceAction(address, "pair", "connecting")
  }
  function disconnectDevice(address) {
    root.runDeviceAction(address, "disconnect", "disconnecting")
  }
  function forgetDevice(address) {
    root.runDeviceAction(address, "forget", "forgetting")
  }
  // Asking for a direction rather than a toggle: the action runs async
  // and the button only moves once BlueZ catches up, so a second click
  // inside that window would re-read the old state and undo the first.
  function togglePower() {
    if (!root.available)
      return
    root.error = ""
    runExec("bluetooth-power", [root.powered ? "off" : "on"], function (ok, out, err) {
      if (!ok)
        root.error = Zerodyne.errorText(err)
    })
  }
  function findDevice(address) {
    var values = root.rawDevices
    for (var i = 0; i < values.length; i++) {
      if (values[i] && values[i].address === address)
        return values[i]
    }
    return null
  }
  function syncPendingActions() {
    var next = {}
    var changed = false
    for (var address in root.pendingActions) {
      var action = root.pendingActions[address]
      var found = root.findDevice(address)
      var done = (action === "connecting" && found && found.connected) || (action
                                                                           === "disconnecting"
                                                                           && found &&
                                                                           !found.connected) || (
            action === "forgetting" && (!found || (!found.paired && !found.bonded &&
                                                   !found.trusted)))

      if (done)
        changed = true
      else
        next[address] = action
    }
    if (changed)
      root.pendingActions = next
  }
  function countConnected(devices) {
    var n = 0
    var values = devices || []
    for (var i = 0; i < values.length; i++) {
      if (values[i] && values[i].connected)
        n += 1
    }
    return n
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
    // Adopt a discovery session that is already running so this close
    // settles it either way.
    if (root.open && root.available && root.adapter.discovering)
      root.owesDiscoveryStop = true
  }
  onRawDevicesChanged: root.syncPendingActions()

  // BlueZ rejects discovery while the adapter is still powering up, and
  // discovery can time out on its own. While open, keep nudging it back
  // on so an enabled adapter is always scanning.
  Timer {
    id: discoveryRetry

    interval: 1000
    repeat: true
    triggeredOnStart: true
    running: root.open && root.available && root.powered && !root.discovering

    onTriggered: {
      root.owesDiscoveryStop = true
      root.adapter.discovering = true
    }
  }
  // The way back down, bound to the confirmed state: a stop issued while
  // a just-fired start is still awaiting confirmation would be swallowed,
  // so a confirmation landing after close re-arms the stop. Bounded so a
  // session some other client holds cannot draw fire forever.
  Timer {
    id: discoveryStop

    property int attempts: 0

    interval: 1000
    repeat: true
    running: !root.open && root.owesDiscoveryStop && root.available && root.adapter.discovering
             === true

    onRunningChanged: {
      if (running)
        attempts = 0
    }
    onTriggered: {
      attempts += 1
      if (attempts > 3) {
        root.owesDiscoveryStop = false
        return
      }
      root.adapter.discovering = false
    }
  }
  // The debt settles the moment BlueZ reports discovery down, so a stale
  // claim never touches a scan another client starts later.
  Connections {
    function onDiscoveringChanged() {
      if (!root.adapter.discovering)
        root.owesDiscoveryStop = false
    }

    target: root.adapter
  }
  Timer {
    id: pendingTimeout

    interval: 20000
    repeat: false

    onTriggered: root.pendingActions = ({})
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
      root.busy = false
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
      // Floor so device names get room instead of eliding after a few
      // characters. Reading own implicitWidth here is loop-free (all
      // implicit inputs are width-independent); forcing width on the
      // children instead would zero their size contribution and collapse
      // the card.
      width: Math.max(implicitWidth, 300)
      spacing: Spacing.sm

      BluetoothHero {
        id: hero

        // Stretches across the card so the power icon pins to the right
        // edge like the network hero actions. Safe: the column floors its
        // own width, so this cannot zero the card's size contribution.
        width: parent.width
        available: root.available
        powered: root.powered
        connectedCount: root.connectedCount
        busy: root.busy

        onPowerRequested: root.togglePower()
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
      BluetoothDeviceList {
        width: parent.width
        devices: root.rawDevices
        pendingActions: root.pendingActions
        emptyText: root.emptyText

        onActivateRow: (address, connected, known) => {
          if (connected)
            root.disconnectDevice(address)
          else if (known)
            root.connectDevice(address)
          else
            root.pairDevice(address)
        }
        onForgetRow: address => root.forgetDevice(address)
      }
    }
  }
}
