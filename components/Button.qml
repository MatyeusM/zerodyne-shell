// Generic button composed from interaction primitives.
//   Button {
//     text: "Apply"
//     onClicked: root.apply()
//   }

import QtQuick
import qs.style
import qs.ui

Item {
  id: root

  property string text: ""
  property bool disabled: false
  property real padding: Spacing.sm
  property real radius: 0
  // Generic surface defaults from `qs.style`; override per use.
  property color background: Color.altSurface
  property color hoverBackground: Color.surface
  property color pressedBackground: Color.background
  property color disabledBackground: Color.surface
  property color textColor: Color.text
  property color hoverTextColor: root.textColor
  property color disabledTextColor: Color.muted
  readonly property alias hovered: click.hovered
  readonly property alias pressed: press.pressed

  signal clicked(var mouse)

  implicitWidth: label.implicitWidth + root.padding * 2
  implicitHeight: label.implicitHeight + root.padding * 2

  Disabled {
    id: disabledState

    disabled: root.disabled
  }
  HoverBackground {
    id: hoverBg

    hovered: click.hovered
    normalBackground: root.background
    hoverBackground: root.hoverBackground
  }
  HoverColor {
    id: hoverFg

    hovered: click.hovered
    normalColor: root.textColor
    hoverColor: root.hoverTextColor
  }
  Pressable {
    id: press

    pressed: click.pressed
  }
  Container {
    anchors.fill: parent
    background: root.disabled ? root.disabledBackground : (press.pressed ? root.pressedBackground :
                                                                           hoverBg.current)
    padding: root.padding
    radius: root.radius

    Text {
      id: label

      // Fill-aligned (not centerIn): keeps the label position independent
      // of the container size so Container's childrenRect sizing stays
      // loop-free. Same visuals for a single label.
      anchors.fill: parent
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      text: root.text
      color: root.disabled ? root.disabledTextColor : hoverFg.current
      font: Typography.display({
                                 "size": Typography.sm
                               })
    }
  }
  Clickable {
    id: click

    anchors.fill: parent
    enabled: disabledState.enabled

    onClicked: mouse => {
      return root.clicked(mouse)
    }
  }
}
