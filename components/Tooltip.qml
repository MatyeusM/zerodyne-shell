// Generic tooltip presentation built on Quickshell PopupWindow.
// Establishes the component and API only: the consumer drives `visible`
// (optionally with its own delay) and positions the popup through the
// inherited `anchor` PopupAnchor object. Not tied to any panel.
//   Tooltip {
//     id: tip
//     anchor.item: button
//     anchor.edges: Qt.BottomEdge
//     anchor.gravity: Qt.BottomEdge
//     text: "Details"
//   }
//   HoverHandler / Clickable hover -> tip.visible = true

import QtQuick
import Quickshell
import qs.style
import qs.ui

PopupWindow {
  id: root

  property string text: ""
  property real padding: Spacing.sm
  property real radius: 0
  property color background: Color.surface
  property color borderColor: Color.muted
  property real borderWidth: 1

  visible: false
  color: "transparent"
  // QsWindow manages width/height itself; drive geometry via implicit size.
  implicitWidth: box.implicitWidth
  implicitHeight: box.implicitHeight

  Container {
    id: box

    anchors.fill: parent
    background: root.background
    padding: root.padding
    radius: root.radius

    Text {
      text: root.text
      color: Color.text
      font: Typography.display({
                                 "size": Typography.sm
                               })
      visible: root.text !== ""
    }
  }
  Border {
    anchors.fill: parent
    borderWidth: root.borderWidth
    borderColor: root.borderColor
    cornerStyle: Border.Rounded
    cornerRadius: root.radius
  }
}
