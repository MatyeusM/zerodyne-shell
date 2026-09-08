import QtQuick
import QtQuick.Layouts
import qs.style
import qs.ui

// Power-profile selector for the performance panel's CPU section: one
// option per available profile, active in primary and bold. Dumb display
// plus user intent; the panel owns the zerodyne set call.
RowLayout {
  id: row

  property var profiles: []
  property string active: ""

  signal selectProfile(name: string)

  spacing: Spacing.sm

  Text {
    Layout.alignment: Qt.AlignVCenter
    text: "Profile"
    color: Color.muted
    font: Typography.display({
                               "size": Typography.sm
                             })
  }
  Repeater {
    model: row.profiles

    Item {
      required property string modelData

      Layout.alignment: Qt.AlignVCenter
      implicitWidth: name.implicitWidth
      implicitHeight: name.implicitHeight

      Clickable {
        id: profileClick

        anchors.fill: parent

        onClicked: row.selectProfile(modelData)
      }
      HoverColor {
        id: profileHover

        hovered: profileClick.hovered
        normalColor: modelData === row.active ? Color.primary : Color.text
        hoverColor: Color.secondary
      }
      Text {
        id: name

        text: modelData
        color: profileHover.current
        font: Typography.display({
                                   "size": Typography.sm,
                                   "weight": modelData === row.active ? Font.Bold : Font.Normal
                                 })
      }
    }
  }
}
