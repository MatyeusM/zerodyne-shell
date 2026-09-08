import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Services.SystemTray
import qs.style
import qs.ui

// Right-side system tray: one cell per active StatusNotifier item.
// Full bar height, between the performance indicator and the power
// button. Plain surface with a bottom border only, so it reads as one
// continuous strip with its neighbors; the performance indicator tucks
// under its left edge and it tucks under power on the right (see Bar.qml).
//
// Left-click activates, middle-click secondary-activates, right-click (or
// left on menu-only items) opens the TrayMenu popup. Passive items stay
// hidden; the module collapses to zero width when the tray is empty.
Item {
  id: root

  property var barScreen: null
  property bool menuOpen: false
  property var activeItem: null
  // Tuck amount Bar.qml uses to slide neighbors under the tray.
  readonly property real chamferCut: Spacing.sm
  readonly property real cellExtent: Spacing.lg
  readonly property real iconSize: Typography.lg
  // Right inset keeps icons off the power button; no left inset — the
  // first cell sits flush against the performance indicator.
  readonly property real rowRight: Spacing.sm
  readonly property var trayItems: root.visibleItems()

  function visibleItems() {
    var values = SystemTray.items.values
    var out = []
    for (var i = 0; i < values.length; i++) {
      if (values[i].status !== Status.Passive) {
        out.push(values[i])
      }
    }
    return out
  }
  // Symbolic icons ship a fixed fill the host is meant to recolor; detect
  // them by the freedesktop "-symbolic" suffix so they can be tinted.
  function isSymbolic(icon) {
    var name = String(icon || "").split("?")[0]
    return name.slice(-9) === "-symbolic"
  }
  function openMenu(item) {
    root.activeItem = item
    root.menuOpen = true
  }
  function handleClick(item, mouse) {
    if (mouse.button === Qt.RightButton) {
      root.openMenu(item)
      return
    }
    if (mouse.button === Qt.MiddleButton) {
      item.secondaryActivate()
      return
    }
    if (item.onlyMenu) {
      root.openMenu(item)
      return
    }
    item.activate()
  }

  implicitWidth: root.trayItems.length === 0 ? 0 : row.implicitWidth + root.rowRight
  implicitHeight: Spacing.lg
  visible: root.trayItems.length > 0

  Container {
    anchors.fill: parent
    background: Color.surface
  }
  Row {
    id: row

    anchors.fill: parent
    anchors.rightMargin: root.rowRight
    spacing: Spacing.xs

    Repeater {
      model: root.trayItems

      TrayCell {
        required property var modelData

        height: row.height
        item: modelData
      }
    }
  }
  Border {
    anchors.fill: parent
    // Bottom edge only: one continuous strip with the neighbors.
    showTop: false
    showLeft: false
    showRight: false
    borderWidth: 2
    borderColor: Color.primary
  }
  TrayMenu {
    open: root.menuOpen
    panelScreen: root.barScreen
    anchorX: root.x + root.width
    activeItem: root.activeItem

    onCloseRequested: {
      root.menuOpen = false
      root.activeItem = null
    }
  }

  // Single tray cell: icon image plus multi-button click handling.
  component TrayCell: Item {
    id: cell

    property var item: null

    implicitWidth: root.cellExtent
    implicitHeight: root.cellExtent

    HoverBackground {
      id: cellHover

      hovered: click.hovered
      normalBackground: "transparent"
      hoverBackground: Color.altSurface
    }
    Rectangle {
      anchors.fill: parent
      color: cellHover.current
    }
    TrayIcon {
      anchors.fill: parent
      anchors.margins: (root.cellExtent - root.iconSize) / 2
      icon: cell.item ? cell.item.icon : ""
    }
    Clickable {
      id: click

      anchors.fill: parent
      acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton

      onClicked: mouse => root.handleClick(cell.item, mouse)
    }
  }

  // Tray icon image, recoloring symbolic icons to the bar foreground so
  // they stay visible on any theme.
  component TrayIcon: Item {
    id: iconRoot

    property string icon: ""
    readonly property bool symbolic: root.isSymbolic(iconRoot.icon)

    Image {
      id: iconImage

      anchors.fill: parent
      fillMode: Image.PreserveAspectFit
      // Decode at physical pixels, otherwise PNG icons upscale blurry.
      sourceSize.width: Math.round(Math.min(width, height) * Screen.devicePixelRatio)
      sourceSize.height: Math.round(Math.min(width, height) * Screen.devicePixelRatio)
      source: iconRoot.icon
      visible: !iconRoot.symbolic
      layer.enabled: iconRoot.symbolic
    }
    MultiEffect {
      anchors.fill: iconImage
      source: iconImage
      visible: iconRoot.symbolic
      colorization: 1.0
      colorizationColor: Color.text
    }
  }
}
