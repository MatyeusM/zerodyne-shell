import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.components
import qs.services
import qs.style
import qs.ui

// Wi-Fi share card: QR matrix plus SSID and password rows. Centered card
// on a fullscreen transparent layer-shell surface (the compositor never
// sizes the card). Scrim click and Escape report closeRequested.
PanelWindow {
  id: root

  property bool open: false
  property var panelScreen: null
  property var qr: null
  property bool loading: false
  property string error: ""

  signal closeRequested

  function refresh() {
    root.error = ""
    root.qr = null
    if (!qrProc.running)
      qrProc.running = true
  }

  // Cap the module pixels so large matrices stay scannable and compact.
  function modulePx() {
    if (root.qr === null || root.qr.size <= 0)
      return 4
    return Math.max(3, Math.floor(200 / root.qr.size))
  }
  function cellOn(index) {
    var row = root.qr.rows[Math.floor(index / root.qr.size)]
    return row[index % root.qr.size] === "1"
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

  Process {
    id: qrProc

    property string err: ""

    command: Zerodyne.execCommand("wifi-qr", [])

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        root.qr = result.ok ? result.data : null
        if (!result.ok)
          root.error = result.error
      }
    }
    stderr: StdioCollector {
      onStreamFinished: qrProc.err = text
    }

    onExited: {
      root.loading = false
      if (qrProc.err !== "") {
        root.qr = null
        root.error = Zerodyne.errorText(qrProc.err)
      }
    }
    onStarted: {
      root.loading = true
      qrProc.err = ""
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
    anchors.centerIn: parent
    background: Color.surface
    padding: Spacing.md
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Container.Chamfered
    chamferTopLeft: true
    chamferTopRight: false
    chamferBottomLeft: false
    chamferBottomRight: true
    chamferSize: Spacing.sm

    Column {
      spacing: Spacing.sm

      Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: root.qr !== null ? root.qr.ssid : (root.loading ? "Loading…" : "Share Wi-Fi")
        color: Color.text
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
      Rectangle {
        id: matrix

        width: root.qr !== null ? root.qr.size * root.modulePx() : 120
        height: width
        color: "white"
        visible: root.qr !== null

        Grid {
          anchors.fill: parent
          columns: root.qr !== null ? root.qr.size : 1

          Repeater {
            model: root.qr !== null ? root.qr.size * root.qr.size : 0

            Rectangle {
              required property int index

              width: root.modulePx()
              height: root.modulePx()
              color: root.cellOn(index) ? Color.background : "transparent"
            }
          }
        }
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
      Text {
        visible: root.qr !== null && root.qr.password !== undefined && root.qr.password !== ""
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: root.qr !== null ? String(root.qr.password) : ""
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
    }
  }
}
