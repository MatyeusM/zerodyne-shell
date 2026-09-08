import QtQuick
import qs.components
import qs.services
import qs.style
import qs.ui

// One bluetooth device: status glyph, name plus state line, forget action
// for remembered devices. Dumb display plus user intent; the panel owns
// all zerodyne calls. Rows carry primitives only, never BlueZ wrappers.
//
// Declaration order is load order: nothing references an id declared
// below it, which hot reload requires.
Item {
  id: row

  property string address: ""
  property string name: ""
  property bool connected: false
  property bool known: false
  property string pending: ""
  property string detail: ""
  property string iconName: ""
  readonly property bool forgettable: row.known
  readonly property string statusText: row.pending !== "" ? row.pending : row.detail

  signal activated
  signal forgetRequested(address: string)

  implicitWidth: nameMeasure.width + glyphMeasure.width + Spacing.sm * 3
  implicitHeight: stack.implicitHeight

  TextMetrics {
    id: nameMeasure

    text: row.name
    font: Typography.display({
                               "size": Typography.sm
                             })
  }
  TextMetrics {
    id: glyphMeasure

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
      implicitHeight: Math.max(glyph.implicitHeight, labels.implicitHeight,
                               forgetSlot.implicitHeight) + Spacing.xxs * 2

      Clickable {
        id: mainClick

        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor

        onClicked: row.activated()
      }
      HoverColor {
        id: hoverFg

        hovered: mainClick.hovered
        normalColor: row.connected ? Color.primary : Color.text
        hoverColor: Color.secondary
      }
      Icon {
        id: glyph

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        glyph: Zerodyne.bluetoothDeviceGlyph(row.iconName, row.connected)
        size: Typography.md
        color: hoverFg.current
      }
      Item {
        id: forgetSlot

        anchors.right: parent.right
        anchors.rightMargin: Spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        width: forgetIcon.size
        height: forgetIcon.size
        visible: row.forgettable

        Clickable {
          id: forgetClick

          anchors.fill: parent

          onClicked: row.forgetRequested(row.address)
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
      Column {
        id: labels

        anchors.left: glyph.right
        anchors.leftMargin: Spacing.sm
        anchors.right: forgetSlot.left
        anchors.rightMargin: Spacing.sm
        anchors.verticalCenter: parent.verticalCenter

        Text {
          width: parent.width
          elide: Text.ElideRight
          text: row.name !== "" ? row.name : row.address
          color: hoverFg.current
          font: Typography.display({
                                     "size": Typography.sm
                                   })
        }
        Text {
          visible: row.statusText !== ""
          width: parent.width
          elide: Text.ElideRight
          text: row.statusText
          color: Color.muted
          font: Typography.mono({
                                  "size": Typography.xs
                                })
        }
      }
    }
  }
}
