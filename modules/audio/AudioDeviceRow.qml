import QtQuick
import QtQuick.Layouts
import qs.components
import qs.style
import qs.ui

// One audio device row, shared verbatim by the OUTPUT and INPUT sections:
// class glyph in a fixed-width column, friendly label elided, bold when
// active. Dumb display plus user intent; the panel owns zerodyne calls.
//
// Declaration order is load order: nothing references an id declared
// below it, which hot reload requires.
Item {
  id: row

  property string glyph: ""
  property string label: ""
  property bool active: false

  signal activated

  implicitHeight: inner.implicitHeight + Spacing.xs

  Clickable {
    id: rowClick

    anchors.fill: parent

    onClicked: row.activated()
  }
  HoverColor {
    id: hoverFg

    hovered: rowClick.hovered
    normalColor: Color.text
    hoverColor: Color.secondary
  }
  RowLayout {
    id: inner

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    anchors.leftMargin: Spacing.sm
    anchors.rightMargin: Spacing.sm
    spacing: Spacing.sm

    Icon {
      Layout.alignment: Qt.AlignVCenter
      glyph: row.glyph
      size: Typography.lg
      color: hoverFg.current
    }
    Text {
      Layout.fillWidth: true
      Layout.alignment: Qt.AlignVCenter
      elide: Text.ElideRight
      text: row.label
      color: hoverFg.current
      font: Typography.display({
                                 "size": Typography.sm,
                                 "weight": row.active ? Font.Bold : Font.Normal
                               })
    }
  }
}
