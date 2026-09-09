import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Io
import qs.components
import qs.services
import qs.style
import qs.ui

// Bar status indicator for the connectivity area: bluetooth on the left,
// network on the right. Each glyph owns its popup; the bar only ever sees
// this item. Wifi state polls zerodyne; bluetooth state is live via
// Quickshell.Bluetooth while all bluetooth actions go through zerodyne.
//
// The bluetooth zone tucks under the clock (see Bar.qml), so the row
// starts past the hidden overlap. Glyphs render through the optically
// centered Icon primitive so MDI shapes of different ink sizes sit
// uniformly.
Item {
  id: root

  property var panelScreen: null
  property bool operateOpen: false
  property bool btOpen: false
  property bool qrOpen: false
  // Parsed `zerodyne status network` payload, or null when offline/broken.
  property var link: null
  readonly property bool connected: root.link !== null
  readonly property var btDevices: Bluetooth.devices ? Bluetooth.devices.values : []
  readonly property bool btAvailable: Bluetooth.defaultAdapter !== null
  readonly property bool btPowered: root.btAvailable && Bluetooth.defaultAdapter.enabled
  readonly property int btCount: root.btDevices.reduce(function (n, d) {
    return n + (d && d.connected ? 1 : 0)
  }, 0)
  // Inset past the clock overlap (its chamfer cut) plus breathing room.
  readonly property real rowLeft: Spacing.sm + Spacing.sm
  readonly property real rowRight: Spacing.sm + Spacing.sm
  readonly property real iconSize: Typography.lg
  readonly property real btZone: root.rowLeft + root.iconSize

  function glyph() {
    if (!root.connected)
      return Zerodyne.linkGlyph("off", 0)
    return Zerodyne.linkGlyph(root.link.type, root.link.signal_pct || 0)
  }
  function btGlyph() {
    return Zerodyne.bluetoothGlyph(root.btPowered, root.btCount > 0)
  }
  function refresh() {
    if (!statusProc.running)
      statusProc.running = true
  }
  // Right-click direction, like the panel button: fire-and-forget, the
  // icon follows adapter state once BlueZ catches up.
  function togglePower() {
    if (execProc.running)
      return
    execProc.command = Zerodyne.execCommand("bluetooth-power", ["toggle"])
    execProc.running = true
  }

  // Fixed width shared with AudioIndicator (Sizing.barIndicatorWidth) so
  // both clock arms match; the narrower glyph pair centers between fill
  // spacers instead of hugging one edge.
  implicitWidth: Sizing.barIndicatorWidth
  implicitHeight: Spacing.lg

  Component.onCompleted: root.refresh()

  Timer {
    interval: 5000
    running: true
    repeat: true

    onTriggered: root.refresh()
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
  // Fire-and-forget power toggle (right-click): no output to collect,
  // the icon follows adapter state once BlueZ catches up.
  Process {
    id: execProc
  }
  Clickable {
    id: btClick

    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    width: root.btZone + Spacing.xs / 2
    acceptedButtons: Qt.LeftButton | Qt.RightButton

    onClicked: mouse => {
      if (mouse.button === Qt.RightButton) {
        root.togglePower()
        return
      }
      root.btOpen = !root.btOpen
      root.operateOpen = false
    }
  }
  Clickable {
    id: wifiClick

    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.left: btClick.right
    anchors.right: parent.right

    onClicked: {
      root.operateOpen = !root.operateOpen
      root.btOpen = false
    }
  }
  HoverColor {
    id: btHover

    hovered: btClick.hovered
    normalColor: Color.text
    hoverColor: Color.secondary
  }
  HoverColor {
    id: wifiHover

    hovered: wifiClick.hovered
    normalColor: Color.text
    hoverColor: Color.secondary
  }
  Container {
    anchors.fill: parent
    background: Color.surface
    cornerStyle: Container.Chamfered
    chamferBottomRight: true
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
        spacing: Spacing.md

        Icon {
          Layout.alignment: Qt.AlignVCenter
          glyph: root.btGlyph()
          size: root.iconSize
          color: root.btPowered ? btHover.current : Color.muted
        }
        Icon {
          Layout.alignment: Qt.AlignVCenter
          glyph: root.glyph()
          size: root.iconSize
          color: root.connected ? wifiHover.current : Color.muted
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
    // No left edge: sits flush against the clock, whose right edge divides.
    showLeft: false
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferBottomRight: true
    chamferSize: Spacing.sm
  }
  NetworkPanel {
    open: root.operateOpen
    panelScreen: root.panelScreen
    // Bar and panel share screen coordinates; align the card's right
    // edge with the indicator's.
    anchorX: root.x + root.width

    onCloseRequested: root.operateOpen = false
    onShowQr: {
      root.operateOpen = false
      root.btOpen = false
      root.qrOpen = true
    }
  }
  BluetoothPanel {
    open: root.btOpen
    panelScreen: root.panelScreen
    anchorX: root.x + root.width

    onCloseRequested: root.btOpen = false
  }
  QrPanel {
    open: root.qrOpen
    panelScreen: root.panelScreen

    onCloseRequested: root.qrOpen = false
  }
}
