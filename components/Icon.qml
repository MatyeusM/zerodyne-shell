import QtQuick
import qs.style

// Simple icon presentation using the mono nerd font, optically centered
// and height-normalized.
//
// MDI/FontAwesome glyphs carry different ink placement AND different ink
// heights, so unscaled glyphs look off-center and unevenly sized. Like
// omarchy's Ui/OpticalGlyph, the painted bounds (not the advance) are
// centered horizontally via tightBoundingRect; on top of that the ink
// height is normalized to a fixed share of the box, so every shape
// renders at the same vertical size. Measured once at the reference
// size (scaling is linear), then applied to the painted font.
//
//   Icon { glyph: "\uf011"; size: 16; color: Color.text }
//
Item {
  id: root

  // Single nerd-font glyph to render. Empty renders nothing.
  property string glyph: ""
  property real size: 16
  property color color: Color.text
  property bool debugBounds: false
  readonly property bool valid: root.glyph !== ""
  readonly property int renderedSize: Math.max(1, Math.round(root.size))
  // Share of the box the ink height is normalized to.
  readonly property real inkShare: 0.72
  readonly property real inkHeight: Math.max(1, inkMetrics.tightBoundingRect.height)
  readonly property real heightScale: Math.min(1.5, Math.max(0.66, root.size * root.inkShare
                                                             / root.inkHeight))
  readonly property int scaledSize: Math.max(1, Math.round(root.renderedSize * root.heightScale))
  // Tight bounds scaled to the painted size for the centering math below.
  readonly property real scaledTightX: inkMetrics.tightBoundingRect.x * root.heightScale
  readonly property real scaledTightWidth: Math.max(1, inkMetrics.tightBoundingRect.width
                                                    * root.heightScale)
  readonly property real horizontalCorrection: ink.implicitWidth / 2 - (root.scaledTightX + root.scaledTightWidth
                                                                        / 2)

  implicitWidth: root.size
  implicitHeight: root.size

  Text {
    id: ink

    anchors.centerIn: parent
    anchors.horizontalCenterOffset: root.horizontalCorrection
    text: root.glyph
    color: root.color
    font: Typography.mono({
                            "size": root.scaledSize
                          })
    renderType: Text.NativeRendering
    visible: root.valid
  }
  TextMetrics {
    id: inkMetrics

    font.family: Typography.monoName
    font.pixelSize: root.renderedSize
    font.weight: Font.Bold
    text: root.glyph
  }
  Rectangle {
    visible: root.debugBounds
    anchors.fill: parent
    color: "transparent"
    border.width: 1
    border.color: "#4488ff"
  }
}
