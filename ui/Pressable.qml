// Reusable pressed/active interaction state.
// Holds no pointer state itself; bind `pressed` to the event source
// (typically `Clickable.pressed`) and let visuals read this item:
//   Pressable {
//     id: press
//     pressed: click.pressed
//   }
//   Container { background: press.pressed ? pressedColor : normalColor }

import QtQuick

QtObject {
  id: root

  // Source of truth for press, typically `Clickable.pressed`.
  property bool pressed: false
}
