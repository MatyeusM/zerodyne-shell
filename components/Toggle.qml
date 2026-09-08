import QtQuick
import qs.style
import qs.ui

// Reusable boolean toggle. Exposes a checked/checkedChanged style API
// with hover/pressed/disabled states and no settings-specific behavior.
//
//   Toggle {
//     checked: settings.enabled
//     onToggled: checked => settings.enabled = checked
//   }
//
Item {
  id: root

  property bool checked: false
  property bool disabled: false

  // Generic metrics; override or theme later without touching logic.
  property real trackWidth: 44
  property real trackHeight: 24
  property real knobSize: 16
  property real knobInset: 4
  property color trackColor: Color.altSurface
  property color hoverTrackColor: Color.surface
  property color checkedColor: Color.primary
  property color knobColor: Color.text
  property color checkedKnobColor: Color.background
  property color disabledTrackColor: Color.surface
  readonly property alias hovered: click.hovered
  readonly property alias pressed: press.pressed

  signal toggled(checked: bool)

  function toggle(): void {
  if (root.disabled)
  return
  root.checked = !root.checked
  root.toggled(root.checked)
}

  implicitWidth: root.trackWidth
  implicitHeight: root.trackHeight

  Disabled {
    id: disabledState

    disabled: root.disabled
  }
  Pressable {
    id: press

    pressed: click.pressed
  }
  HoverBackground {
    id: hoverBg

    hovered: click.hovered
    normalBackground: root.trackColor
    hoverBackground: root.hoverTrackColor
  }
  Rectangle {
    id: track

    anchors.fill: parent
    radius: height / 2
    color: root.disabled ? root.disabledTrackColor : (root.checked ? root.checkedColor : (
                                                                       press.pressed
                                                                       ? root.hoverTrackColor :
                                                                         hoverBg.current))

    Behavior on color {
      ColorAnimation {
        duration: 120
      }
    }

    Rectangle {
      id: knob

      width: root.knobSize
      height: root.knobSize
      radius: height / 2
      anchors.verticalCenter: parent.verticalCenter
      x: root.checked ? parent.width - width - root.knobInset : root.knobInset
      color: root.checked ? root.checkedKnobColor : root.knobColor

      Behavior on x {
        NumberAnimation {
          duration: 120
          easing.type: Easing.OutCubic
        }
      }
      Behavior on color {
        ColorAnimation {
          duration: 120
        }
      }
    }
  }
  Clickable {
    id: click

    anchors.fill: parent
    enabled: disabledState.enabled

    onClicked: root.toggle()
  }
}
