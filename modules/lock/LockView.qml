import QtQuick
import Quickshell
import qs.components
import qs.style

// Password view for the session lock. Deliberately small: a three-line clock
// header and one field. Instantiated on every surface so keys always have a
// focused input; showVisuals keeps the clock and dots on the main output only.
// No fingerprint, idle, or suspend logic.
Item {
  id: root

  property string failureMessage: ""
  property bool authenticating: false
  property bool showVisuals: true
  // Shared with LockService so typing on any output renders on the main one.
  property string passwordText: ""
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

  signal passwordEdited(string text)
  signal submitRequested(string password)

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
    visible: root.showVisuals

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
      // Always focusable, even on the dark surfaces. Only the main output
      // shows the dots, and only after typing has begun.
      opacity: root.showVisuals && field.text.length > 0 ? 1 : 0

      onTextChanged: {
        if (field.text !== root.passwordText)
          root.passwordEdited(field.text)
      }
      onAccepted: {
        var value = field.text
        field.text = ""
        root.submitRequested(value)
      }
      Component.onCompleted: {
        field.text = root.passwordText
        field.forceFocus()
      }

      Connections {
        function onPasswordTextChanged() {
          if (field.text !== root.passwordText)
            field.text = root.passwordText
        }

        target: root
      }
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
