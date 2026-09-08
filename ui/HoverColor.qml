// Reusable foreground-color hover behavior (text, glyph, line color).
// Holds no pointer state itself; bind `hovered` to a Clickable:
//   HoverColor {
//     id: hoverFg
//     hovered: click.hovered
//     normalColor: Color.text
//     hoverColor: Color.primary
//   }
//   Text { color: hoverFg.current }

import QtQuick

QtObject {
  id: root

  // Source of truth for hover, typically `Clickable.hovered`.
  property bool hovered: false
  property color normalColor: "transparent"
  property color hoverColor: "transparent"
  // Color to apply. Bind the consumer's color property to this.
  readonly property color current: root.hovered ? root.hoverColor : root.normalColor
}
