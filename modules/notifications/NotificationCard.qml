import QtQuick
import Quickshell.Services.Notifications
import qs.style
import qs.ui

// Toast card: app header, summary, body on the bar surface with a neon
// ring, chamfered bottom-left. Click fires the sender's default action
// when it has one, otherwise just closes. Hover tints the summary so the
// card reads as clickable.
Item {
  id: root

  property string app: ""
  property string summary: ""
  property string body: ""
  property int urgency: NotificationUrgency.Normal
  property bool hasDefault: false
  property alias hovered: click.hovered
  readonly property bool critical: root.urgency === NotificationUrgency.Critical

  signal closeRequested
  signal defaultRequested

  implicitWidth: Sizing.sm
  implicitHeight: content.implicitHeight + Spacing.sm * 2

  Container {
    anchors.fill: parent
    background: Color.surface
    cornerStyle: Container.Chamfered
    chamferBottomLeft: true
    chamferSize: Spacing.sm

    Column {
      id: content

      anchors.fill: parent
      anchors.margins: Spacing.sm
      spacing: Spacing.xxs

      Text {
        visible: root.app !== ""
        width: parent.width
        elide: Text.ElideRight
        text: root.app
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.xs
                                 })
      }
      Text {
        width: parent.width
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
        text: root.summary
        color: summaryHover.current
        font: Typography.display()
      }
      Text {
        visible: root.body !== ""
        width: parent.width
        wrapMode: Text.WordWrap
        textFormat: Text.StyledText
        text: root.body
        color: Color.muted
        font: Typography.display({
                                   "size": Typography.sm
                                 })
      }
    }
  }
  Border {
    anchors.fill: parent
    borderWidth: 2
    borderColor: root.critical ? Color.tertiary : Color.primary
    cornerStyle: Border.Chamfered
    chamferBottomLeft: true
    chamferSize: Spacing.sm
  }
  Clickable {
    id: click

    anchors.fill: parent

    onClicked: {
      if (root.hasDefault) {
        root.defaultRequested()
      } else {
        root.closeRequested()
      }
    }
  }
  HoverColor {
    id: summaryHover

    hovered: click.hovered
    normalColor: Color.text
    hoverColor: Color.secondary
  }
}
