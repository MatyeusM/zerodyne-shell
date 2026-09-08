import QtQuick
import qs.style

// Dumb segmented bar for the performance panel (stays in
// modules/performance until a second consumer exists). The panel computes
// every segment; this only paints them contiguously over a track, with the
// overlay value centered on top.
//
// Contrast trick: the label paints twice — once in the track color, once
// clipped to the solid region in the fill color — so it reads on both.
// `fillEnd` marks the solid region end (ghost segments past it keep the
// track-colored label).
//
//   PerformanceBar {
//     fills: [{fraction: 0.4, color: Color.primary}]
//     fillEnd: 0.4
//     overlayText: "42%"
//     fillTextColor: Color.background
//   }
Item {
  id: root

  property var fills: []
  property real fillEnd: 0
  property string overlayText: ""
  property color fillTextColor: Color.background
  property color trackTextColor: Color.text
  readonly property real clipEnd: Math.max(0, Math.min(1, root.fillEnd))

  implicitHeight: Spacing.lg

  Rectangle {
    anchors.fill: parent
    color: Color.altSurface
  }
  Row {
    anchors.fill: parent

    Repeater {
      model: root.fills

      Rectangle {
        required property var modelData

        width: Math.max(0, Math.min(1, modelData.fraction || 0)) * parent.width
        height: parent.height
        color: modelData.color || "transparent"
      }
    }
  }
  Text {
    anchors.centerIn: parent
    text: root.overlayText
    color: root.trackTextColor
    font: Typography.mono({
                            "size": Typography.sm
                          })
  }
  Item {
    width: parent.width * root.clipEnd
    height: parent.height
    clip: true

    Text {
      width: root.width
      height: parent.height
      verticalAlignment: Text.AlignVCenter
      horizontalAlignment: Text.AlignHCenter
      text: root.overlayText
      color: root.fillTextColor
      font: Typography.mono({
                              "size": Typography.sm
                            })
    }
  }
}
