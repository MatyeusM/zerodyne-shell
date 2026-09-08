// Reusable content container with background and padding.
//   Container {
//     background: Color.surface
//     padding: Spacing.sm
//     radius: 4
//     Text { text: "hello" }
//   }
// Borders are not handled here; compose a sibling Border for the ring:
//   Item {
//     Container { id: box; anchors.fill: parent; background: ...; radius: 4 }
//     Border { anchors.fill: parent; cornerStyle: Border.Rounded; cornerRadius: 4 }
//   }
//
// Implicit size follows the content's childrenRect. Keep content position
// independent of the container size (fill-aligned with alignment properties
// rather than centerIn): size-relative positioning feeds back into
// childrenRect and produces binding loops.

import QtQuick
import QtQuick.Shapes

Item {
  id: root

  enum CornerStyle {
    Square,
    Rounded,
    Chamfered
  }

  property color background: "transparent"
  property real radius: 0
  property real padding: 0
  // Chamfered background. Mirrors ui/Border values so a sibling Border
  // ring and this fill stay in sync; the fill cuts the same triangles
  // the Border diagonals replace, otherwise the square body pokes past
  // the chamfer. Only used when `cornerStyle` is Chamfered.
  property int cornerStyle: Container.Square
  property bool chamferTopLeft: false
  property bool chamferTopRight: false
  property bool chamferBottomLeft: false
  property bool chamferBottomRight: false
  property real chamferSize: 6
  readonly property bool chamfered: root.cornerStyle === Container.Chamfered
  readonly property real cutTopLeft: root.chamfered && root.chamferTopLeft ? root.chamferSize : 0
  readonly property real cutTopRight: root.chamfered && root.chamferTopRight ? root.chamferSize : 0
  readonly property real cutBottomLeft: root.chamfered && root.chamferBottomLeft ? root.chamferSize :
                                                                                   0
  readonly property real cutBottomRight: root.chamfered && root.chamferBottomRight
                                         ? root.chamferSize : 0
  // Child content. Aliased to the padded content holder's data.
  default property alias content: contentItem.data

  implicitWidth: contentItem.childrenRect.width + root.padding * 2
  implicitHeight: contentItem.childrenRect.height + root.padding * 2

  Rectangle {
    id: frame

    anchors.fill: parent
    color: root.background
    radius: root.radius
    visible: !root.chamfered
  }
  Shape {
    anchors.fill: parent
    antialiasing: true
    visible: root.chamfered

    ShapePath {
      fillColor: root.background
      strokeWidth: 0
      strokeColor: "transparent"

      PathMove {
        x: root.cutTopLeft
        y: 0
      }
      PathLine {
        x: root.width - root.cutTopRight
        y: 0
      }
      PathLine {
        x: root.width
        y: root.cutTopRight
      }
      PathLine {
        x: root.width
        y: root.height - root.cutBottomRight
      }
      PathLine {
        x: root.width - root.cutBottomRight
        y: root.height
      }
      PathLine {
        x: root.cutBottomLeft
        y: root.height
      }
      PathLine {
        x: 0
        y: root.height - root.cutBottomLeft
      }
      PathLine {
        x: 0
        y: root.cutTopLeft
      }
      PathLine {
        x: root.cutTopLeft
        y: 0
      }
    }
  }
  Item {
    id: contentItem

    anchors.fill: parent
    anchors.margins: root.padding
  }
}
