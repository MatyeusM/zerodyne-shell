import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.components
import qs.services
import qs.style
import qs.ui

// Power menu: centered card with one large action per row-entry — LOGOUT,
// LOCK, RESTART, SHUTDOWN — over a dimmed fullscreen scrim. Scrim click
// and Escape report closeRequested. Actions are fire-and-forget through
// zerodyne (`power <action>`); the panel closes on click without waiting.
PanelWindow {
  id: root

  property bool open: false
  property var panelScreen: null
  readonly property real glyphSize: Spacing.xxl
  readonly property var actions: [
    {
      action: "logout",
      label: "LOGOUT",
      glyph: Zerodyne.glyphPowerLogout
    },
    {
      action: "lock",
      label: "LOCK",
      glyph: Zerodyne.glyphLock
    },
    {
      action: "restart",
      label: "RESTART",
      glyph: Zerodyne.glyphPowerRestart
    },
    {
      action: "shutdown",
      label: "SHUTDOWN",
      glyph: Zerodyne.glyphPowerShutdown
    }
  ]

  signal closeRequested

  function activate(action) {
    if (execProc.running)
      return
    execProc.command = Zerodyne.execCommand("power", [action])
    execProc.running = true
    root.closeRequested()
  }

  visible: root.open
  anchors.top: true
  anchors.bottom: true
  anchors.left: true
  anchors.right: true
  color: Qt.rgba(0, 0, 0, 0.6)
  exclusionMode: ExclusionMode.Ignore
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
  screen: root.panelScreen

  Process {
    id: execProc
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
    padding: Spacing.lg
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Container.Chamfered
    chamferTopLeft: true
    chamferTopRight: false
    chamferBottomLeft: false
    chamferBottomRight: true
    chamferSize: Spacing.sm

    RowLayout {
      spacing: Spacing.lg

      Repeater {
        model: root.actions

        Item {
          required property var modelData

          implicitWidth: entry.implicitWidth
          implicitHeight: entry.implicitHeight

          Clickable {
            id: entryClick

            anchors.fill: parent

            onClicked: root.activate(modelData.action)
          }
          HoverColor {
            id: entryHover

            hovered: entryClick.hovered
            normalColor: Color.text
            hoverColor: Color.secondary
          }
          ColumnLayout {
            id: entry

            spacing: Spacing.sm

            Icon {
              Layout.alignment: Qt.AlignHCenter
              glyph: modelData.glyph
              size: root.glyphSize
              color: entryHover.current
            }
            Text {
              Layout.alignment: Qt.AlignHCenter
              text: modelData.label
              color: entryHover.current
              font: Typography.display({
                                         "size": Typography.sm
                                       })
            }
          }
        }
      }
    }
  }
}
