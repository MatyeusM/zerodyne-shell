import QtQuick
import QtQuick.Layouts
import qs.components
import qs.services
import qs.style
import qs.ui

// One visible wifi network: strength glyph, SSID, right-aligned forget
// and lock icons. Dumb display plus user intent; the panel owns all
// zerodyne calls and tells the row when to prompt.
//
// Declaration order is load order: nothing references an id declared
// below it, which hot reload requires.
Item {
  id: row

  property string ssid: ""
  property int signal: 0
  property string security: "open"
  property bool inUse: false
  property bool known: false
  property bool prompt: false
  property bool busy: false
  readonly property bool secured: row.security !== "open"
  readonly property bool clickable: !row.inUse
  readonly property bool forgettable: row.known && !row.inUse

  signal connectRequested(ssid: string, password: string)
  signal forgetRequested(ssid: string)
  signal cancelPrompt

  implicitWidth: ssidMeasure.width + sigMeasure.width + Spacing.sm * 3
  implicitHeight: stack.implicitHeight

  // Intrinsic width for the card: worst-case signal plus SSID measure.
  TextMetrics {
    id: ssidMeasure

    text: row.ssid
    font: Typography.display({
                               "size": Typography.sm
                             })
  }
  TextMetrics {
    id: sigMeasure

    text: "100%"
    font: Typography.mono({
                            "size": Typography.xs
                          })
  }
  Column {
    id: stack

    width: row.width
    spacing: 0

    Item {
      id: main

      width: parent.width
      implicitHeight: name.implicitHeight + Spacing.xxs * 2

      Clickable {
        id: mainClick

        anchors.fill: parent
        enabled: row.clickable
        cursorShape: row.clickable ? Qt.PointingHandCursor : Qt.ArrowCursor

        onClicked: row.connectRequested(row.ssid, "")
      }
      HoverColor {
        id: hoverFg

        hovered: mainClick.hovered && row.clickable
        normalColor: row.inUse ? Color.primary : Color.text
        hoverColor: Color.secondary
      }
      RowLayout {
        anchors.fill: parent
        anchors.rightMargin: Spacing.sm
        spacing: Spacing.sm

        Icon {
          Layout.alignment: Qt.AlignVCenter
          glyph: Zerodyne.linkGlyph("wifi", row.signal)
          size: Typography.sm
          color: hoverFg.current
        }
        Text {
          id: name

          Layout.fillWidth: true
          Layout.alignment: Qt.AlignVCenter
          elide: Text.ElideRight
          text: row.ssid
          color: hoverFg.current
          font: Typography.display({
                                     "size": Typography.sm
                                   })
        }
        Item {
          Layout.alignment: Qt.AlignVCenter
          Layout.preferredWidth: forgetIcon.size
          Layout.preferredHeight: forgetIcon.size
          visible: row.forgettable

          Clickable {
            id: forgetClick

            anchors.fill: parent

            onClicked: row.forgetRequested(row.ssid)
          }
          HoverColor {
            id: forgetHover

            hovered: forgetClick.hovered
            normalColor: Color.muted
            hoverColor: Color.secondary
          }
          Icon {
            id: forgetIcon

            anchors.centerIn: parent
            glyph: Zerodyne.glyphForget
            size: 16
            color: forgetHover.current
          }
        }
        Text {
          Layout.alignment: Qt.AlignVCenter
          visible: row.secured
          text: Zerodyne.glyphLock
          color: Color.muted
          font: Typography.mono({
                                  "size": Typography.xs
                                })
        }
      }
    }
    Row {
      visible: row.prompt
      spacing: Spacing.sm

      TextField {
        id: passphrase

        width: 160
        echoMode: TextInput.Password
        placeholderText: "Password"
      }
      Button {
        text: "Join"
        disabled: row.busy

        onClicked: row.connectRequested(row.ssid, passphrase.text)
      }
      Button {
        padding: Spacing.xxs
        text: "Cancel"
        background: "transparent"
        hoverBackground: "transparent"
        pressedBackground: "transparent"
        disabledBackground: "transparent"
        textColor: Color.muted
        hoverTextColor: Color.secondary

        onClicked: {
          passphrase.text = ""
          row.cancelPrompt()
        }
      }
    }
  }
}
