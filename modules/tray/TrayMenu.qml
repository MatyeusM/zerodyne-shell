import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Wayland
import qs.components
import qs.style
import qs.ui

// Tray context-menu popup: fullscreen transparent layer-shell surface
// (ExclusionMode.Ignore, so it reserves no space) with the menu card
// floating below the bar, right edge under the tray.
//
// QsMenuOpener renders the DBus menu in-process: the shell has no
// UseQApplication pragma, so entry.display() platform menus are refused
// and every menu must go through this card. One nested level is kept as
// its own live opener: a child entry is owned by its parent opener's
// model, so collapsing to a single opener would destroy the entry shown.
PanelWindow {
  id: root

  property bool open: false
  property var panelScreen: null
  // Bar x-coordinate to right-align the card to (the tray's right edge:
  // bar and panel share the screen coordinate space).
  property real anchorX: 0
  property var activeItem: null
  property var submenuStack: []
  readonly property int submenuDepth: root.submenuStack.length
  readonly property string currentTitle: root.submenuDepth > 0
                                         ? root.submenuStack[root.submenuDepth - 1].title : ""
  readonly property var currentChildren: root.submenuDepth > 0
                                         ? root.submenuStack[root.submenuDepth - 1].opener.children :
                                           menuOpener.children
  readonly property int menuCount: root.currentChildren ? root.currentChildren.values.length : 0
  readonly property real menuMinWidth: Spacing.xxl * 4
  readonly property real menuMaxHeight: Spacing.lg * 10
  readonly property real rowHeight: Spacing.lg

  signal closeRequested

  function resetMenu() {
    // Clear the reactive stack before teardown so no binding reads a
    // partially-destroyed opener, then destroy deepest first: an inner
    // opener's entry is owned by its parent's children model.
    var openers = root.submenuStack
    root.submenuStack = []
    for (var i = openers.length - 1; i >= 0; i--) {
      openers[i].opener.destroy()
    }
  }
  function enterSubmenu(entry, title) {
    var opener = submenuOpenerComponent.createObject(root, {
                                                       menu: entry
                                                     })
    if (!opener) {
      return
    }
    var stack = root.submenuStack.slice()
    var level = {}
    level.opener = opener
    level.title = title
    stack.push(level)
    root.submenuStack = stack
    menuFlick.contentY = 0
  }
  function leaveSubmenu() {
    if (root.submenuStack.length === 0) {
      return
    }
    var stack = root.submenuStack.slice()
    var top = stack.pop()
    root.submenuStack = stack
    top.opener.destroy()
    menuFlick.contentY = 0
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

  onOpenChanged: root.resetMenu()
  onActiveItemChanged: root.resetMenu()

  QsMenuOpener {
    id: menuOpener

    menu: root.activeItem ? root.activeItem.menu : null
  }
  Component {
    id: submenuOpenerComponent

    QsMenuOpener {
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
    // Below the bar, right edge under the tray.
    anchors.top: parent.top
    anchors.topMargin: Spacing.lg + Spacing.xs
    x: Math.max(0, root.anchorX - implicitWidth)
    background: Color.surface
    padding: Spacing.sm
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferTopLeft: true
    chamferBottomRight: true
    chamferSize: Spacing.sm

    Column {
      id: layout

      // Self-read floors the card: implicit inputs are width-independent
      // (delegates size from inner content, never from this width).
      width: Math.max(implicitWidth, root.menuMinWidth)
      spacing: 0

      Item {
        visible: root.submenuDepth > 0
        width: layout.width
        implicitWidth: backLabel.implicitWidth + Spacing.sm * 2 + root.rowHeight
        implicitHeight: root.rowHeight

        Clickable {
          id: backClick

          anchors.fill: parent

          onClicked: root.leaveSubmenu()
        }
        HoverBackground {
          id: backHover

          hovered: backClick.hovered
          normalBackground: "transparent"
          hoverBackground: Color.altSurface
        }
        Rectangle {
          anchors.fill: parent
          color: backHover.current
        }
        Text {
          anchors.left: parent.left
          anchors.leftMargin: Spacing.sm
          anchors.verticalCenter: parent.verticalCenter
          text: "‹"
          color: Color.muted
          font: Typography.display({
                                     size: Typography.md
                                   })
        }
        Text {
          id: backLabel

          anchors.left: parent.left
          anchors.leftMargin: Spacing.sm + root.rowHeight / 2
          anchors.right: parent.right
          anchors.rightMargin: Spacing.sm
          anchors.verticalCenter: parent.verticalCenter
          text: root.currentTitle
          color: Color.text
          font: Typography.display({
                                     size: Typography.sm
                                   })
          elide: Text.ElideRight
        }
      }
      Text {
        visible: root.menuCount === 0
        width: layout.width
        leftPadding: Spacing.sm
        rightPadding: Spacing.sm
        topPadding: Spacing.xs
        bottomPadding: Spacing.xs
        text: "No actions"
        color: Color.muted
        font: Typography.display({
                                   size: Typography.sm
                                 })
      }
      Flickable {
        id: menuFlick

        width: layout.width
        height: Math.min(menuColumn.implicitHeight, root.menuMaxHeight)
        visible: root.menuCount > 0
        contentWidth: width
        contentHeight: menuColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        interactive: contentHeight > height

        ScrollBar.vertical: AutoScrollbar {
          id: menuBar
        }

        Column {
          id: menuColumn

          width: menuFlick.width - (menuFlick.interactive ? menuBar.gutter : 0)
          spacing: 0

          Repeater {
            model: root.currentChildren

            delegate: Item {
              id: menuRow

              required property var modelData
              required property int index
              readonly property string rowText: String(modelData.text || "")
              readonly property bool isSeparator: modelData.isSeparator
              readonly property bool enabled: modelData.enabled
              readonly property bool hasChildren: modelData.hasChildren
              readonly property bool checked: modelData.buttonType !== QsMenuButtonType.None
                                              && modelData.checkState === Qt.Checked

              width: menuColumn.width
              implicitWidth: entryRow.implicitWidth + Spacing.sm * 2 + root.rowHeight
              implicitHeight: menuRow.isSeparator ? Spacing.sm : root.rowHeight
              opacity: menuRow.enabled || menuRow.isSeparator ? 1.0 : 0.45

              Clickable {
                id: rowClick

                anchors.fill: parent
                enabled: !menuRow.isSeparator && menuRow.enabled

                onClicked: {
                  if (menuRow.hasChildren) {
                    root.enterSubmenu(menuRow.modelData, menuRow.rowText)
                  } else {
                    menuRow.modelData.triggered()
                    root.closeRequested()
                  }
                }
              }
              HoverBackground {
                id: rowHover

                hovered: rowClick.hovered
                normalBackground: "transparent"
                hoverBackground: Color.altSurface
              }
              Rectangle {
                anchors.fill: parent
                visible: !menuRow.isSeparator
                color: rowHover.current
              }
              Separator {
                visible: menuRow.isSeparator
                anchors.verticalCenter: parent.verticalCenter
              }
              Row {
                id: entryRow

                visible: !menuRow.isSeparator
                anchors.left: parent.left
                anchors.leftMargin: Spacing.sm
                anchors.verticalCenter: parent.verticalCenter
                spacing: Spacing.xs

                Icon {
                  anchors.verticalCenter: parent.verticalCenter
                  visible: menuRow.checked
                  glyph: "\uf00c"
                  size: Typography.sm
                  color: Color.text
                }
                Image {
                  anchors.verticalCenter: parent.verticalCenter
                  visible: String(menuRow.modelData.icon || "") !== ""
                  width: Spacing.md
                  height: Spacing.md
                  fillMode: Image.PreserveAspectFit
                  sourceSize.width: Math.round(Spacing.md * Screen.devicePixelRatio)
                  sourceSize.height: Math.round(Spacing.md * Screen.devicePixelRatio)
                  source: menuRow.modelData.icon || ""
                }
                Text {
                  anchors.verticalCenter: parent.verticalCenter
                  text: menuRow.rowText
                  color: Color.text
                  font: Typography.display({
                                             size: Typography.sm
                                           })
                }
              }
              Text {
                visible: !menuRow.isSeparator && menuRow.hasChildren
                anchors.right: parent.right
                anchors.rightMargin: Spacing.sm
                anchors.verticalCenter: parent.verticalCenter
                text: "›"
                color: Color.muted
                font: Typography.display({
                                           size: Typography.md
                                         })
              }
            }
          }
        }
      }
    }
  }
}
