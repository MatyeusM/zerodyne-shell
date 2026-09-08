// Generic surface for larger UI regions.
// Composes Container (background + padding) with a Border ring rather than
// duplicating either behavior. Keep `radius` and `cornerRadius` in sync when
// both background and ring are visible.
//   Panel {
//     padding: Spacing.md
//     Text { text: "content" }
//   }

import QtQuick
import qs.style
import qs.ui

Item {
  id: root

  property color background: Color.surface
  property real padding: 0
  property real radius: 0
  property bool borderVisible: true
  property real borderWidth: 0
  property color borderColor: Color.muted
  property int cornerStyle: Border.Square
  property real cornerRadius: 0
  property bool chamferTopLeft: false
  property bool chamferTopRight: false
  property bool chamferBottomLeft: false
  property bool chamferBottomRight: false
  property real chamferSize: 6
  default property alias content: box.content

  implicitWidth: box.implicitWidth
  implicitHeight: box.implicitHeight

  Container {
    id: box

    anchors.fill: parent
    background: root.background
    padding: root.padding
    radius: root.radius
    cornerStyle: root.cornerStyle
    chamferTopLeft: root.chamferTopLeft
    chamferTopRight: root.chamferTopRight
    chamferBottomLeft: root.chamferBottomLeft
    chamferBottomRight: root.chamferBottomRight
    chamferSize: root.chamferSize
  }
  Border {
    anchors.fill: parent
    borderVisible: root.borderVisible
    borderWidth: root.borderWidth
    borderColor: root.borderColor
    cornerStyle: root.cornerStyle
    cornerRadius: root.cornerRadius
    chamferTopLeft: root.chamferTopLeft
    chamferTopRight: root.chamferTopRight
    chamferBottomLeft: root.chamferBottomLeft
    chamferBottomRight: root.chamferBottomRight
    chamferSize: root.chamferSize
  }
}
