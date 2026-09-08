import QtQuick
import qs.style
import qs.ui

// Band pinning selector: muted label on the left, options right-aligned.
// The selected option shows primary; the rest hover secondary.
Item {
  id: root

  property var band: null
  // Hidden while disconnected: band pinning needs an active link and
  // setting it then has no effect.
  property var link: null

  signal selectBand(value: string)

  visible: root.link !== null && root.band !== null
  implicitWidth: bandLabel.implicitWidth + opts.implicitWidth + Spacing.sm
  implicitHeight: Math.max(bandLabel.implicitHeight, opts.implicitHeight)

  Text {
    id: bandLabel

    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    text: "Band"
    color: Color.muted
    font: Typography.display({
                               "size": Typography.sm
                             })
  }
  Row {
    id: opts

    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    spacing: Spacing.sm

    Repeater {
      model: root.band !== null ? ["auto"].concat(root.band.available || []) : []

      Text {
        required property string modelData

        text: modelData === "auto" ? "Auto" : modelData + " GHz"
        color: modelData === root.band.selected ? Color.primary : bandHover.current
        font: Typography.display({
                                   "size": Typography.sm
                                 })

        Clickable {
          id: bandClick

          anchors.fill: parent

          onClicked: root.selectBand(modelData)
        }
        HoverColor {
          id: bandHover

          hovered: bandClick.hovered
          normalColor: Color.text
          hoverColor: Color.secondary
        }
      }
    }
  }
}
