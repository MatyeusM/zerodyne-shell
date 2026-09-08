import QtQuick
import qs.components
import qs.services
import qs.style
import qs.ui

// Connection hero: state glyph, two-line SSID/detail block, QR and
// Disconnect actions pinned to the right edge, all vertically centered.
// Display only; the panel wires the signals.
Item {
  id: root

  property var link: null

  signal showQr
  signal disconnectRequested

  function glyph() {
    if (root.link === null)
      return Zerodyne.linkGlyph("off", 0)
    return Zerodyne.linkGlyph(root.link.type, root.link.signal_pct || 0)
  }
  function title() {
    if (root.link === null)
      return "Disconnected"
    if (root.link.type === "ethernet")
      return "Ethernet"
    return root.link.ssid || "Wi-Fi"
  }
  function subtitle() {
    if (root.link === null)
      return "no route"
    if (root.link.type === "ethernet") {
      var speed = root.link.speed_mb ? root.link.speed_mb + " Mb/s" : ""
      return [root.link.iface, root.link.ip, speed].filter(function (p) {
        return p !== "" && p !== undefined
      }).join(" · ")
    }
    var pct = root.link.signal_pct !== undefined ? root.link.signal_pct + "%" : ""
    var freq = root.link.freq_mhz ? Math.round(root.link.freq_mhz / 100) / 10 + " GHz" : ""
    return [pct, freq, root.link.iface].filter(function (p) {
      return p !== "" && p !== undefined
    }).join(" · ")
  }

  implicitWidth: symbol.implicitWidth + labels.implicitWidth + actions.implicitWidth + Spacing.sm
                 * 2

  implicitHeight: Math.max(symbol.implicitHeight, labels.implicitHeight, actions.implicitHeight)

  Icon {
    id: symbol

    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    glyph: root.glyph()
    size: Typography.xl
    color: Color.primary
  }
  Column {
    id: labels

    anchors.left: symbol.right
    anchors.leftMargin: Spacing.sm
    anchors.verticalCenter: parent.verticalCenter

    Text {
      text: root.title()
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
  Row {
    id: actions

    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    spacing: Spacing.sm

    Item {
      width: qrIcon.size
      height: qrIcon.size

      Clickable {
        id: qrClick

        anchors.fill: parent

        onClicked: root.showQr()
      }
      HoverColor {
        id: qrHover

        hovered: qrClick.hovered
        normalColor: Color.text
        hoverColor: Color.secondary
      }
      Icon {
        id: qrIcon

        anchors.centerIn: parent
        glyph: Zerodyne.glyphQr
        size: 20
        color: qrHover.current
      }
    }
    Item {
      width: discIcon.size
      height: discIcon.size
      visible: root.link !== null && root.link.type === "wifi"

      Clickable {
        id: discClick

        anchors.fill: parent

        onClicked: root.disconnectRequested()
      }
      HoverColor {
        id: discHover

        hovered: discClick.hovered
        normalColor: Color.text
        hoverColor: Color.secondary
      }
      Icon {
        id: discIcon

        anchors.centerIn: parent
        glyph: Zerodyne.glyphWifiOff
        size: 20
        color: discHover.current
      }
    }
  }
}
