// Border overlay with configurable width, color, sides, and corners.
// Place as a child of the item it frames and anchor it to fill:
//   Border {
//     anchors.fill: parent
//     borderWidth: 2
//     borderColor: Color.primary
//     showTop: false
//     chamferBottomLeft: true
//     chamferBottomRight: true
//     chamferSize: Spacing.sm
//   }

// Geometry responsibility stays with the consumer: this item draws only the
// ring/edges and never sizes, pads, or positions anything.
//
// Rendering: crisp axis-aligned edge rectangles plus antialiased Shape
// diagonals for chamfers. Chamfer endpoints come from the style/Chamfer
// singleton, which hand-calculates the cut so the stroke stays inside the
// frame.

import QtQuick
import QtQuick.Shapes
import qs.style

Item {
  id: root

  enum CornerStyle {
    Square,
    Rounded,
    Chamfered
  }

  // Master switch for the border. `borderWidth: 0` also hides it.
  property bool borderVisible: true
  property real borderWidth: 1
  // Neutral default; consumers typically bind a `qs.style` token.
  property color borderColor: "transparent"
  property int cornerStyle: Border.Square
  // Used only when `cornerStyle` is Rounded on a full ring (see below).
  property real cornerRadius: 0

  // Per-side visibility. All true draws the full ring.
  property bool showTop: true
  property bool showBottom: true
  property bool showLeft: true
  property bool showRight: true

  // Per-corner chamfer cuts. Only used when `cornerStyle` is Chamfered:
  // adjoining edges stop short by `chamferSize` and a diagonal joins them.
  // A chamfer draws only when both adjoining sides are shown.
  property bool chamferTopLeft: false
  property bool chamferTopRight: false
  property bool chamferBottomLeft: false
  property bool chamferBottomRight: false
  property real chamferSize: 6
  readonly property bool linesOn: root.borderVisible && root.borderWidth > 0
  readonly property bool fullRing: root.showTop && root.showBottom && root.showLeft
                                   && root.showRight

  readonly property bool chamfered: root.cornerStyle === Border.Chamfered
  readonly property real cut: root.chamfered ? root.chamferSize : 0
  // Rounded rendering is kept for the simple full-ring case (a Rectangle
  // ring cannot do partial sides or chamfers); anything else falls back to
  // square edges.
  readonly property bool roundedRing: root.cornerStyle === Border.Rounded && root.fullRing && root.cut
                                      <= 0
  readonly property real halfWidth: root.borderWidth / 2

  // Centerline endpoints for each diagonal, from style/Chamfer.
  readonly property var cutTopLeft: Chamfer.diagonal(0, root.width, root.height, root.cut,
                                                     root.halfWidth)
  readonly property var cutTopRight: Chamfer.diagonal(1, root.width, root.height, root.cut,
                                                      root.halfWidth)
  readonly property var cutBottomLeft: Chamfer.diagonal(2, root.width, root.height, root.cut,
                                                        root.halfWidth)
  readonly property var cutBottomRight: Chamfer.diagonal(3, root.width, root.height, root.cut,
                                                         root.halfWidth)

  // Full-ring rounded case, preserving the original Border behavior.
  Rectangle {
    anchors.fill: parent
    color: "transparent"
    border.width: root.borderWidth
    border.color: root.borderColor
    radius: root.roundedRing ? root.cornerRadius : 0
    visible: root.roundedRing && root.linesOn
  }

  // Per-side edges. Adjacent edges overlap at square corners, which is
  // invisible since they share one color.
  Rectangle {
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.leftMargin: Chamfer.edgeInset(root.chamferTopLeft && root.chamfered, root.chamferSize)
    anchors.right: parent.right
    anchors.rightMargin: Chamfer.edgeInset(root.chamferTopRight && root.chamfered, root.chamferSize)
    implicitHeight: root.borderWidth
    color: root.borderColor
    visible: !root.roundedRing && root.linesOn && root.showTop
  }
  Rectangle {
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.leftMargin: Chamfer.edgeInset(root.chamferBottomLeft && root.chamfered,
                                          root.chamferSize)
    anchors.right: parent.right
    anchors.rightMargin: Chamfer.edgeInset(root.chamferBottomRight && root.chamfered,
                                           root.chamferSize)
    implicitHeight: root.borderWidth
    color: root.borderColor
    visible: !root.roundedRing && root.linesOn && root.showBottom
  }
  Rectangle {
    anchors.left: parent.left
    anchors.top: parent.top
    anchors.topMargin: Chamfer.edgeInset(root.chamferTopLeft && root.chamfered, root.chamferSize)
    anchors.bottom: parent.bottom
    anchors.bottomMargin: Chamfer.edgeInset(root.chamferBottomLeft && root.chamfered,
                                            root.chamferSize)
    implicitWidth: root.borderWidth
    color: root.borderColor
    visible: !root.roundedRing && root.linesOn && root.showLeft
  }
  Rectangle {
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.topMargin: Chamfer.edgeInset(root.chamferTopRight && root.chamfered, root.chamferSize)
    anchors.bottom: parent.bottom
    anchors.bottomMargin: Chamfer.edgeInset(root.chamferBottomRight && root.chamfered,
                                            root.chamferSize)
    implicitWidth: root.borderWidth
    color: root.borderColor
    visible: !root.roundedRing && root.linesOn && root.showRight
  }

  // Antialiased diagonal cuts, one stroked path per chamfered corner.
  Shape {
    anchors.fill: parent
    antialiasing: true
    visible: !root.roundedRing && root.linesOn && root.chamfered && root.cut > 0

    ShapePath {
      strokeWidth: root.chamferTopLeft && root.showTop && root.showLeft ? root.borderWidth : 0
      strokeColor: root.borderColor
      fillColor: "transparent"
      capStyle: ShapePath.RoundCap

      PathMove {
        x: root.cutTopLeft.ax
        y: root.cutTopLeft.ay
      }
      PathLine {
        x: root.cutTopLeft.bx
        y: root.cutTopLeft.by
      }
    }
    ShapePath {
      strokeWidth: root.chamferTopRight && root.showTop && root.showRight ? root.borderWidth : 0
      strokeColor: root.borderColor
      fillColor: "transparent"
      capStyle: ShapePath.RoundCap

      PathMove {
        x: root.cutTopRight.ax
        y: root.cutTopRight.ay
      }
      PathLine {
        x: root.cutTopRight.bx
        y: root.cutTopRight.by
      }
    }
    ShapePath {
      strokeWidth: root.chamferBottomLeft && root.showBottom && root.showLeft ? root.borderWidth : 0
      strokeColor: root.borderColor
      fillColor: "transparent"
      capStyle: ShapePath.RoundCap

      PathMove {
        x: root.cutBottomLeft.ax
        y: root.cutBottomLeft.ay
      }
      PathLine {
        x: root.cutBottomLeft.bx
        y: root.cutBottomLeft.by
      }
    }
    ShapePath {
      strokeWidth: root.chamferBottomRight && root.showBottom && root.showRight ? root.borderWidth :
                                                                                  0
      strokeColor: root.borderColor
      fillColor: "transparent"
      capStyle: ShapePath.RoundCap

      PathMove {
        x: root.cutBottomRight.ax
        y: root.cutBottomRight.ay
      }
      PathLine {
        x: root.cutBottomRight.bx
        y: root.cutBottomRight.by
      }
    }
  }
}
