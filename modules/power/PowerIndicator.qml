import QtQuick
import QtQuick.Layouts
import qs.components
import qs.services
import qs.style
import qs.ui

// Far-right power button: one shutdown glyph on the tray's own surface —
// no chamfer, no side borders — so it reads as part of the tray, and
// still looks like the tray when the tray is empty. The tray tucks under
// it (see Bar.qml) so both bottom borders join; this item stays on top
// via z. Click toggles the power menu; the action itself is fire-and-
// forget through zerodyne.
Item {
  id: root

  property var panelScreen: null
  property bool powerOpen: false
  // Tuck amount Bar.qml uses to slide the tray under this item.
  readonly property real chamferCut: Spacing.sm
  // No left inset: this sits flush against the tray cells as one of them.
  readonly property real rowLeft: Spacing.sm
  readonly property real rowRight: Spacing.sm
  readonly property real iconSize: Typography.lg

  implicitWidth: root.rowLeft + root.iconSize + root.rowRight
  implicitHeight: Spacing.lg

  Clickable {
    id: click

    anchors.fill: parent

    onClicked: root.powerOpen = !root.powerOpen
  }
  HoverColor {
    id: hoverFg

    hovered: click.hovered
    normalColor: Color.text
    hoverColor: Color.secondary
  }
  Container {
    anchors.fill: parent
    background: Color.surface

    RowLayout {
      anchors.fill: parent
      anchors.leftMargin: root.rowLeft
      anchors.rightMargin: root.rowRight

      Icon {
        Layout.alignment: Qt.AlignVCenter
        glyph: Zerodyne.glyphPowerShutdown
        size: root.iconSize
        color: hoverFg.current
      }
    }
  }
  Border {
    anchors.fill: parent
    // Bottom edge only: no side borders, so this reads as a continuation
    // of the tray rather than its own indicator. The tray tucks under it
    // (see Bar.qml) and their bottom borders join into one line.
    showTop: false
    showLeft: false
    showRight: false
    borderWidth: 2
    borderColor: Color.primary
  }
  PowerPanel {
    open: root.powerOpen
    panelScreen: root.panelScreen

    onCloseRequested: root.powerOpen = false
  }
}
