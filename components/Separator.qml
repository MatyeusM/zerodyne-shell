// Simple visual divider. Orientation follows Qt.Horizontal/Vertical.
//
// Drop-in usage spans the parent (full width when horizontal, full height
// when vertical); set explicit anchors to constrain it instead, which
// replace these defaults:
//   Separator {
//   }

import QtQuick
import qs.style

Item {
  id: root

  property int orientation: Qt.Horizontal
  property real thickness: 1
  property color color: Color.muted
  readonly property bool isHorizontal: root.orientation === Qt.Horizontal

  anchors.left: root.isHorizontal ? parent.left : undefined
  anchors.right: root.isHorizontal ? parent.right : undefined
  anchors.top: root.isHorizontal ? undefined : parent.top
  anchors.bottom: root.isHorizontal ? undefined : parent.bottom
  implicitWidth: root.isHorizontal ? 0 : root.thickness
  implicitHeight: root.isHorizontal ? root.thickness : 0

  Rectangle {
    anchors.centerIn: parent
    width: root.isHorizontal ? parent.width : root.thickness
    height: root.isHorizontal ? root.thickness : parent.height
    color: root.color
  }
}
