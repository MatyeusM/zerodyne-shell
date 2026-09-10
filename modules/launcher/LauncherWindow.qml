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
// No bar button: opens via IPC from a Hyprland keybind, on the focused
// monitor's screen (shell.activeScreen). Launching runs entry.execute();
// the window closes on launch, Escape, or scrim click.
PanelWindow {
  id: root

  property var dialogScreen: null
  property bool open: false
  readonly property real iconSize: Typography.xl
  // Movement-gated hover-select: typing rebuilds the list under a static
  // cursor, and the fresh delegate's hovered would otherwise steal the
  // keyboard selection. Only the scrim feed arms it: the scrim is
  // fullscreen, static, and never rebuilt, so its position events mean
  // real pointer travel, while delegate creation/rebuild motion under a
  // static cursor is ignored. Typing, opening, or an external model
  // refresh disarms and re-anchors; crossing hoverThreshold arms again.
  property bool hoverArmed: false
  property real hoverAnchorX: -1
  property real hoverAnchorY: -1
  property real lastMouseX: -1
  property real lastMouseY: -1
  readonly property real hoverThreshold: Spacing.xs

  function noteMouseMove(px, py) {
    root.lastMouseX = px
    root.lastMouseY = py
    if (root.hoverAnchorX < -0.5) {
      root.hoverAnchorX = px
      root.hoverAnchorY = py
      return
    }
    var dx = px - root.hoverAnchorX
    var dy = py - root.hoverAnchorY
    if (dx * dx + dy * dy > root.hoverThreshold * root.hoverThreshold) {
      root.hoverArmed = true
    }
  }
  function disarmHover() {
    root.hoverArmed = false
    root.hoverAnchorX = root.lastMouseX
    root.hoverAnchorY = root.lastMouseY
  }

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
      root.disarmHover()
      Qt.callLater(function () {
        field.forceFocus()
      })
    }
  }

  Launcher {
    id: launcher
  }
  Connections {
    function onResultsChanged() {
      root.disarmHover()
    }

    target: launcher
  }
  Clickable {
    id: scrimClick

    anchors.fill: parent

    onClicked: root.open = false
    // Sole arming source (see above): this sensor never rebuilds or moves.
    onPositionChanged: mouse => {
      var p = scrimClick.mapToItem(scrimClick.parent, mouse.x, mouse.y)
      root.noteMouseMove(p.x, p.y)
    }
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

    // Fixed list budget: the field plus a full page of maxResults rows, so
    // the dialog size never changes as the filter narrows (fewer rows
    // just leave empty space at the bottom; rows never shift for layout
    // reasons, only list content changes). Own child (field) + earlier
    // sibling (launcher) refs: safe declare-before-use targets.
    Column {
      anchors.left: parent.left
      anchors.right: parent.right
      spacing: Spacing.xs
      height: field.implicitHeight + Spacing.xs + launcher.maxResults * Spacing.xl + (launcher.maxResults
                                                                                      - 1) * Spacing.xs

      TextField {
        id: field

        width: parent.width
        placeholderText: "Search applications"

        onTextChanged: {
          launcher.query = field.text
          root.disarmHover()
        }
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

            // Selection only, never arming: delegate creation/rebuild
            // motion under a static cursor must not arm hover-select
            // (that was the ~500ms-after-typing steal: the freq-load
            // refresh rebuilds delegates, and a creation-time position
            // event armed just before hovered fired).
            onHoveredChanged: {
              if (rowClick.hovered && root.hoverArmed) {
                launcher.selected = row.index
              }
            }
            onPositionChanged: {
              if (root.hoverArmed) {
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

            // Selection is the only indicator: hover feeds selection
            // (movement-gated above), never highlights directly, so a
            // static cursor under a rebuilt list shows nothing extra.
            hovered: launcher.selected === row.index
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
