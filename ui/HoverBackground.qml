// Reusable background-color hover behavior (fills, surfaces).
// Same concept as HoverColor, but for background fills. Holds no pointer
// state itself; bind `hovered` to a Clickable:
//   HoverBackground {
//     id: hoverBg
//     hovered: click.hovered
//     normalBackground: Color.altSurface
//     hoverBackground: Color.surface
//   }
//   Container { background: hoverBg.current }

import QtQuick

QtObject {
  id: root

  // Source of truth for hover, typically `Clickable.hovered`.
  property bool hovered: false
  property color normalBackground: "transparent"
  property color hoverBackground: "transparent"
  // Background to apply. Bind the consumer's background property to this.
  readonly property color current: root.hovered ? root.hoverBackground : root.normalBackground
}
