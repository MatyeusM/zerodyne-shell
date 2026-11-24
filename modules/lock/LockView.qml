import QtQuick
import Quickshell
import qs.components
import qs.style

// Password view for the session lock. Deliberately small: a three-line clock
// header and one field. Only the DP-2 surface hosts this; every other output
// is left dark by LockService. No fingerprint, idle, or suspend logic.
Item {
  id: root

  property string failureMessage: ""
  property bool authenticating: false
  // The field only becomes visible once the user actually starts typing.
  property bool passwordEntered: false
  readonly property var weekdayNames: ["SUNDAY", "MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY",
    "FRIDAY", "SATURDAY"]
  readonly property var monthNames: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
    "Nov", "Dec"]
  readonly property string timeText: ("0" + clock.hours).slice(-2) + ":" + ("0"
                                                                            + clock.minutes).slice(
                                       -2)
  readonly property string dayText: root.weekdayNames[clock.date.getDay()]
  readonly property string dateText: clock.date.getDate() + " " + root.monthNames[clock.date.getMonth(
                                                                                    )] + " "
                                     + clock.date.getFullYear()
  // Fixed row heights: the header's centre can then be placed at 1/3 of the
  // screen without reading back the column's own geometry.
  readonly property real headerHeight: Math.round(Typography.lockTime * 1.05) + Math.round(Typography.lockDay
                                                                                           * 1.05) + Math.round(
                                         Typography.lockDate * 1.05)
  readonly property real dotSpacing: dotMetrics.advanceWidth * 0.33

  signal submitPassword(string password)

  onAuthenticatingChanged: {
    if (!root.authenticating)
      Qt.callLater(field.forceFocus)
  }

  // Declared first: the bindings below read it, and forward references break
  // on hot reload.
  SystemClock {
    id: clock

    precision: SystemClock.Minutes
  }
  // Natural (unspaced) advances for the two lines we letter-space. Qt adds
  // letterSpacing after every character, including the last, so each line
  // spreads the shortfall over its gaps and offsets the padded box by half
  // a spacing to centre the visible ink.
  TextMetrics {
    id: dayMetrics

    text: root.dayText
    font.family: Typography.displayName
    font.pixelSize: Typography.lockDay
    font.weight: Font.Bold
  }
  TextMetrics {
    id: dateMetrics

    text: root.dateText
    font.family: Typography.displayName
    font.pixelSize: Typography.lockDate
    font.weight: Font.Bold
  }
  // Password dots use the same font as TextField. 33% of a dot's advance
  // keeps them from touching; the extra left padding cancels the trailing
  // spacing Qt adds after the last glyph so the dots stay centred.
  TextMetrics {
    id: dotMetrics

    text: "\u25CF"
    font.family: Typography.monoName
    font.pixelSize: Typography.sm
    font.weight: Font.Bold
  }
  // Tight three-line block: equal measured widths, zero extra spacing, each
  // line vertically centred in its own row. Its centre sits at 1/3 height.
  Column {
    id: header

    anchors.horizontalCenter: parent.horizontalCenter
    anchors.top: parent.top
    anchors.topMargin: Math.max(Spacing.lg, parent.height / 3 - root.headerHeight / 2)
    spacing: 0

    Text {
      anchors.horizontalCenter: parent.horizontalCenter
      text: root.timeText
      color: Theme.primary
      font.family: Typography.displayName
      font.pixelSize: Typography.lockTime
      font.weight: Font.Bold
      height: Math.round(Typography.lockTime * 1.05)
      verticalAlignment: Text.AlignVCenter
    }
    Text {
      // Qt appends letterSpacing after the last glyph too. Spread it over
      // the gaps only, then shift the box right by half a spacing so the
      // visible ink — not the padded box — is centred.
      readonly property real fittedSpacing: Math.max(0, (Typography.lockHeaderWidth
                                                         - dayMetrics.advanceWidth) / Math.max(1,
                                                                                               root.dayText.length
                                                                                               - 1))

      anchors.horizontalCenter: parent.horizontalCenter
      anchors.horizontalCenterOffset: fittedSpacing / 2
      text: root.dayText
      color: Theme.text
      font.family: Typography.displayName
      font.pixelSize: Typography.lockDay
      font.weight: Font.Bold
      font.letterSpacing: fittedSpacing
      height: Math.round(Typography.lockDay * 1.05)
      verticalAlignment: Text.AlignVCenter
    }
    Text {
      readonly property real fittedSpacing: Math.max(0, (Typography.lockHeaderWidth
                                                         - dateMetrics.advanceWidth) / Math.max(1,
                                                                                                root.dateText.length
                                                                                                - 1))

      anchors.horizontalCenter: parent.horizontalCenter
      anchors.horizontalCenterOffset: fittedSpacing / 2
      text: root.dateText
      color: Theme.text
      font.family: Typography.displayName
      font.pixelSize: Typography.lockDate
      font.weight: Font.Bold
      font.letterSpacing: fittedSpacing
      height: Math.round(Typography.lockDate * 1.05)
      verticalAlignment: Text.AlignVCenter
    }
  }
  Column {
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.top: header.bottom
    anchors.topMargin: Spacing.xl
    spacing: Spacing.sm

    TextField {
      id: field

      width: 320
      height: 44
      placeholderText: "Password"
      echoMode: TextInput.Password
      passwordCharacter: "\u25CF"
      horizontalAlignment: TextInput.AlignHCenter
      letterSpacing: root.dotSpacing
      textLeftPadding: root.dotSpacing
      disabled: root.authenticating
      // Stays in the layout and keeps focus, but is invisible until the
      // first keystroke lands.
      opacity: root.passwordEntered ? 1 : 0

      onTextChanged: root.passwordEntered = field.text.length > 0
      onAccepted: {
        var value = field.text
        field.text = ""
        root.submitPassword(value)
      }
      Component.onCompleted: field.forceFocus()
    }
    Text {
      anchors.horizontalCenter: parent.horizontalCenter
      text: root.failureMessage
      color: Theme.tertiary
      font.family: Typography.displayName
      font.pixelSize: Typography.sm
      visible: root.failureMessage !== ""
    }
  }
}
