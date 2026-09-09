import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Widgets
import qs.components
import qs.style
import qs.ui

// Rofi replacement: centered dialog with an autofocused search field and
// fuzzy results carrying icons plus comment subtitles. Keyboard (type,
// Up/Down, Enter, Esc) and mouse (hover-select, click-launch) both work.
// No bar button: opens via IPC from a Hyprland keybind, on the bar's
// screen. Launching runs entry.execute(); the window closes on launch,
// Escape, or scrim click.
PanelWindow {
  id: root

  property var dialogScreen: null
  property bool open: false
  readonly property real iconSize: Typography.xl

  visible: root.open
  anchors.top: true
  anchors.bottom: true
  anchors.left: true
  anchors.right: true
  color: "transparent"
  exclusionMode: ExclusionMode.Ignore
  screen: root.dialogScreen
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

  onOpenChanged: {
    if (root.open) {
      field.text = ""
      launcher.refresh()
      Qt.callLater(function () {
        field.forceFocus()
      })
    }
  }

  Launcher {
    id: launcher
  }
  Clickable {
    anchors.fill: parent

    onClicked: root.open = false
  }
  Panel {
    id: dialog

    anchors.centerIn: parent
    width: Sizing.xl
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

    Keys.onEscapePressed: root.open = false
    Keys.onUpPressed: launcher.move(-1)
    Keys.onDownPressed: launcher.move(1)

    Column {
      anchors.left: parent.left
      anchors.right: parent.right
      spacing: Spacing.xs

      TextField {
        id: field

        width: parent.width
        placeholderText: "Search applications"

        onTextChanged: launcher.query = field.text
        onAccepted: {
          launcher.launchSelected()
          root.open = false
        }
      }
      Repeater {
        model: launcher.results

        delegate: Item {
          id: row

          required property var modelData
          required property int index

          width: parent.width
          height: Spacing.xl

          Rectangle {
            anchors.fill: parent
            color: launcher.selected === row.index ? Color.altSurface : "transparent"
          }
          IconImage {
            anchors.left: parent.left
            anchors.leftMargin: Spacing.sm
            anchors.verticalCenter: parent.verticalCenter
            implicitSize: root.iconSize
            asynchronous: true
            source: Quickshell.iconPath(row.modelData.icon || "")
            visible: status === Image.Ready
            backer.sourceSize.width: Math.round(root.iconSize * Screen.devicePixelRatio)
            backer.sourceSize.height: Math.round(root.iconSize * Screen.devicePixelRatio)
          }
          Column {
            anchors.left: parent.left
            anchors.leftMargin: Spacing.sm + root.iconSize + Spacing.sm
            anchors.right: parent.right
            anchors.rightMargin: Spacing.sm
            anchors.verticalCenter: parent.verticalCenter

            Text {
              width: parent.width
              elide: Text.ElideRight
              text: row.modelData.name || ""
              color: nameHover.current
              font: Typography.display({
                                         "size": Typography.sm
                                       })
            }
            Text {
              visible: (row.modelData.comment || "") !== ""
              width: parent.width
              elide: Text.ElideRight
              textFormat: Text.PlainText
              text: row.modelData.comment || ""
              color: Color.muted
              font: Typography.display({
                                         "size": Typography.xs
                                       })
            }
          }
          Clickable {
            id: rowClick

            anchors.fill: parent

            onHoveredChanged: {
              if (rowClick.hovered) {
                launcher.selected = row.index
              }
            }
            onClicked: {
              launcher.launchAt(row.index)
              root.open = false
            }
          }
          HoverColor {
            id: nameHover

            hovered: rowClick.hovered || launcher.selected === row.index
            normalColor: Color.text
            hoverColor: Color.primary
          }
        }
      }
      Text {
        visible: launcher.results.length === 0
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: "No matching application"
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
    }
  }
  IpcHandler {
    function toggle(): string {
      root.open = !root.open
      return "ok"
    }
    function open(): string {
      root.open = true
      return "ok"
    }
    function close(): string {
      root.open = false
      return "ok"
    }

    target: "launcher"
  }
}
