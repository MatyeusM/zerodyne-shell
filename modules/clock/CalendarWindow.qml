import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.components
import qs.style
import qs.ui

// Standalone calendar surface for the DateClock: a fullscreen transparent
// layer-shell panel (ExclusionMode.Ignore, so it reserves no space) with
// the month card floating below the bar.
//
// Deliberately NOT a Hyprland-managed FloatingWindow: the compositor takes
// over true-window sizing, while here quickshell owns the card geometry.
// A read-out, not a picker: no selection, no settings. Outside click and
// Escape report `closeRequested`; the owner holds the open state.
PanelWindow {
  id: root

  property bool open: false
  property var calendarScreen: null
  readonly property date today: sysclock.date
  readonly property int year: root.today.getFullYear()
  readonly property int month: root.today.getMonth()
  readonly property var locale: Qt.locale()
  readonly property string monthLabel: root.locale.monthName(root.month, Locale.LongFormat) + " "
                                       + root.year
  // Qt weeks start at Monday=1..Sunday=7; JS dates use Sunday=0..Saturday=6.
  readonly property int weekStart: root.locale.firstDayOfWeek % 7
  readonly property int daysInMonth: new Date(root.year, root.month + 1, 0).getDate()
  readonly property int leadBlanks: (new Date(root.year, root.month, 1).getDay() - root.weekStart
                                     + 7) % 7

  signal closeRequested

  function dayAt(index) {
    var day = index - root.leadBlanks + 1
    return day >= 1 && day <= root.daysInMonth ? day : 0
  }

  // Column index to Qt day-of-week for locale day names.
  function qtWeekday(index) {
    var jsDay = (root.weekStart + index) % 7
    return jsDay === 0 ? 7 : jsDay
  }

  visible: root.open
  anchors.top: true
  anchors.bottom: true
  anchors.left: true
  anchors.right: true
  color: "transparent"
  exclusionMode: ExclusionMode.Ignore
  // Layer-shell surfaces receive no key events by default; OnDemand lets
  // the focused card handle Escape without grabbing the keyboard outright.
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
  screen: root.calendarScreen

  SystemClock {
    id: sysclock

    // Hourly is plenty for tracking the current day.
    precision: SystemClock.Hours
  }
  MouseArea {
    anchors.fill: parent

    onClicked: root.closeRequested()
  }
  Item {
    anchors.fill: parent
    focus: root.open

    Keys.onEscapePressed: root.closeRequested()
  }
  Panel {
    id: frame

    // Below the bar: bar height (Spacing.lg) plus a small gap.
    anchors.top: parent.top
    anchors.topMargin: Spacing.lg + Spacing.xs
    anchors.horizontalCenter: parent.horizontalCenter
    background: Color.surface
    padding: Spacing.md
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferTopLeft: true
    chamferBottomRight: true
    chamferSize: Spacing.sm

    Column {
      spacing: Spacing.sm

      Text {
        width: grid.width
        horizontalAlignment: Text.AlignHCenter
        text: root.monthLabel
        color: Color.text
        font: Typography.display()
      }
      Grid {
        id: grid

        columns: 7
        spacing: Spacing.xxs

        Repeater {
          model: 7

          Text {
            required property int index

            width: Spacing.lg
            horizontalAlignment: Text.AlignHCenter
            text: root.locale.dayName(root.qtWeekday(index), Locale.NarrowFormat)
            color: Color.muted
            font: Typography.mono({
                                    "size": Typography.xs
                                  })
          }
        }
        Repeater {
          model: 42

          Container {
            required property int index
            readonly property int day: root.dayAt(index)

            width: Spacing.lg
            height: Spacing.lg
            background: day !== 0 && day === root.today.getDate() ? Color.primary : "transparent"

            Text {
              anchors.fill: parent
              horizontalAlignment: Text.AlignHCenter
              verticalAlignment: Text.AlignVCenter
              text: day === 0 ? "" : String(day)
              color: day !== 0 && day === root.today.getDate() ? Color.background : Color.text
              font: Typography.mono({
                                      "size": Typography.xs
                                    })
            }
          }
        }
      }
    }
  }
}
