// Thin scrollbar for panel lists (tray menu, wifi/bluetooth lists).
// Appears only while its Flickable overflows; while it does, subtract
// `gutter` from the content width so rows never slide under the bar.
//
//   ListView {
//     ScrollBar.vertical: AutoScrollbar {
//       id: bar
//     }
//   }

import QtQuick
import QtQuick.Controls
import qs.style

ScrollBar {
  id: root

  // Whether the attached Flickable currently overflows. Attached bar
  // values parent to their Flickable, so this needs no wiring. Driven
  // explicitly because stock AsNeeded bookkeeping does not reliably
  // hide the bar again after content shrinks to fit (a stale visible
  // bar at size 1 survives a steam -> discord menu switch).
  property bool needed: root.parent && root.parent.contentHeight !== undefined
                        ? root.parent.contentHeight > root.parent.height : true

  // Bar width plus breathing room: the single number consumers
  // reserve beside their content while overflowing.
  readonly property real gutter: root.contentItem.implicitWidth + Spacing.xs

  policy: root.needed ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff

  contentItem: Rectangle {
    implicitWidth: 2
    color: Color.text
  }
}
