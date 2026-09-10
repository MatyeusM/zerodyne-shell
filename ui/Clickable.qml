// Reusable pointer/click interaction primitive.
// Place as a child of a visual item and anchor it to the interactive area:
//   Clickable {
//     anchors.fill: parent
//     onClicked: root.activated()
//   }

import QtQuick

// This item is the single pointer-event source for a widget. When composing
// with Pressable, HoverColor, HoverBackground, HoverAppear, or Disabled,
// bind those primitives to this item's state instead of adding more
// MouseAreas (overlapping MouseAreas would compete for events).
// Movement-only observation stays here too (positionChanged): a second
// MouseArea would fight for hover/click delivery.
Item {
  // The inherited `enabled` property gates interaction. When composing with
  // Disabled, bind it as `enabled: disabledState.enabled`.

  id: root

  // Hover state. Bind `hovered` on HoverColor/HoverBackground/HoverAppear here.
  readonly property alias hovered: sensor.containsMouse
  // Pressed state. Bind `pressed` on Pressable here.
  readonly property alias pressed: sensor.pressed
  // Pointer cursor shown while hovering. Defaults to the pointing hand.
  property alias cursorShape: sensor.cursorShape
  // Mouse buttons that trigger activation. Defaults to the left button.
  property alias acceptedButtons: sensor.acceptedButtons

  signal clicked(var mouse)
  signal doubleClicked(var mouse)
  signal pressAndHold(var mouse)
  // Raw pointer movement inside the area. Used for movement-gated hover
  // (e.g. launcher hover-select arms only after the pointer really moves,
  // so list rebuilds under a static cursor never steal selection).
  signal positionChanged(var mouse)

  MouseArea {
    id: sensor

    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton

    onClicked: mouse => {
      return root.clicked(mouse)
    }
    onPositionChanged: mouse => {
      return root.positionChanged(mouse)
    }
    onDoubleClicked: mouse => {
      return root.doubleClicked(mouse)
    }
    onPressAndHold: mouse => {
      return root.pressAndHold(mouse)
    }
  }
}
