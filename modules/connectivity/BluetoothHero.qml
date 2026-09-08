import QtQuick
import qs.components
import qs.services
import qs.style
import qs.ui

// Bluetooth hero: state glyph, title plus status block, power icon action
// pinned to the right edge like the network hero actions, all vertically
// centered. Display only; the panel wires the signals.
Item {
  id: root

  property bool available: false
  property bool powered: false
  property int connectedCount: 0
  property bool busy: false

  signal powerRequested

  function subtitle() {
    if (!root.available)
      return "no adapter"
    if (!root.powered)
      return "turned off"
    if (root.connectedCount > 0)
      return root.connectedCount === 1 ? "1 connected" : root.connectedCount + " connected"
    return "on"
  }

  implicitWidth: symbol.implicitWidth + labels.implicitWidth + powerSlot.implicitWidth + Spacing.sm
                 * 2
  implicitHeight: Math.max(symbol.implicitHeight, labels.implicitHeight, powerSlot.implicitHeight)

  Icon {
    id: symbol

    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    glyph: Zerodyne.bluetoothGlyph(root.powered, root.connectedCount > 0)
    size: Typography.xl
    color: root.powered ? Color.primary : Color.muted
  }
  Item {
    id: powerSlot

    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    width: powerIcon.size
    height: powerIcon.size
    visible: root.available

    Clickable {
      id: powerClick

      anchors.fill: parent
      enabled: !root.busy

      onClicked: root.powerRequested()
    }
    HoverColor {
      id: powerHover

      hovered: powerClick.hovered
      normalColor: Color.text
      hoverColor: Color.secondary
    }
    Icon {
      id: powerIcon

      anchors.centerIn: parent
      glyph: root.powered ? Zerodyne.glyphBluetoothOff : Zerodyne.glyphBluetooth
      size: 20
      color: powerHover.current
    }
  }
  Column {
    id: labels

    anchors.left: symbol.right
    anchors.leftMargin: Spacing.sm
    anchors.verticalCenter: parent.verticalCenter

    Text {
      text: "Bluetooth"
      color: Color.text
      font: Typography.display({
                                 "size": Typography.sm
                               })
    }
    Text {
      text: root.subtitle()
      color: Color.muted
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
  }
}
