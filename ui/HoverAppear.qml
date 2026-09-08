// Reusable appear-on-hover behavior.
// The element is visually absent when not hovered and appears on hover.
// Exposes state only; the consumer decides how to present it and may add
// its own animation (no animation style is hard-coded here):
//   HoverAppear {
//     id: appear
//     hovered: click.hovered
//   }
//   Item {
//     opacity: appear.opacity
//     visible: appear.visible
//     Behavior on opacity { NumberAnimation { duration: 120 } }
//   }

import QtQuick

QtObject {
  id: root

  // Source of truth for hover, typically `Clickable.hovered`.
  property bool hovered: false
  // Opacity endpoints. Override either end to restyle the fade.
  property real hiddenOpacity: 0
  property real shownOpacity: 1
  // Opacity to apply. Bind the consumer's `opacity` to this.
  readonly property real opacity: root.hovered ? root.shownOpacity : root.hiddenOpacity
  // Presence flag. Bind the consumer's `visible` to this so a hidden
  // element stops participating in hit-testing and layout.
  readonly property bool visible: root.hovered
}
