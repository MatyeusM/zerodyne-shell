import QtQuick
import Quickshell
import qs.style
import qs.ui

// Centered clock module. Shows hh:mm, updates once per minute, and toggles
// a standalone calendar window on click. Calendar rendering lives in
// CalendarWindow.qml; this file only owns the toggle state.
Item {
  id: root

  property bool calendarOpen: false
  property var calendarScreen: null
  readonly property string timeText: Qt.formatTime(clock.date, "hh:mm")
  // Chamfer cut shared by the background and the ring. Bar.qml reads this
  // to tuck the indicator under the clock by exactly the cut, so the two
  // bottom borders join into one line.
  readonly property real chamferCut: Spacing.sm

  implicitWidth: measure.width + Spacing.lg * 2
  // Short vertical padding: the element must stay within the bar height,
  // otherwise the layer surface clips the bottom border.
  implicitHeight: label.implicitHeight + Spacing.xxs * 2

  SystemClock {
    id: clock

    precision: SystemClock.Minutes
  }
  Clickable {
    id: click

    anchors.fill: parent

    onClicked: root.calendarOpen = !root.calendarOpen
  }
  HoverColor {
    id: hoverFg

    hovered: click.hovered
    normalColor: Color.primary
    hoverColor: Color.secondary
  }
  // Rajdhani is not monospaced, so measure a fixed widest-case sample.
  // Keeps the element (and its borders) stable as digits change.
  TextMetrics {
    id: measure

    text: "00:00"
    font: label.font
  }
  Container {
    anchors.fill: parent
    background: Color.surface
    cornerStyle: Container.Chamfered
    chamferBottomLeft: true
    chamferBottomRight: true
    chamferSize: root.chamferCut

    Text {
      id: label

      // Fill-aligned (not centerIn): keeps any childrenRect sizing loop-free.
      anchors.fill: parent
      anchors.leftMargin: Spacing.lg
      anchors.rightMargin: Spacing.lg
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      text: root.timeText
      color: hoverFg.current
      font: Typography.display()
    }
  }
  Border {
    anchors.fill: parent
    showTop: false
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferBottomLeft: true
    chamferBottomRight: true
    chamferSize: root.chamferCut
  }
  CalendarWindow {
    open: root.calendarOpen
    calendarScreen: root.calendarScreen

    onCloseRequested: root.calendarOpen = false
  }
}
