import QtQuick
import QtQuick.Layouts
import qs.style

// One tabular performance row: two-line label column (title plus muted
// detail, the network hero pattern) beside a dominant bar carrying its
// own centered value. Dumb display; the panel computes everything.
//
// The label column is a plain Column with explicit widths: a nested
// ColumnLayout with fillWidth texts reports an implicit width that makes
// the outer RowLayout surrender the bar's share (verified empirically —
// the bar collapses to ~2px).
RowLayout {
  id: row

  property string title: ""
  property string subtitle: ""
  property var fills: []
  property real fillEnd: 0
  property string overlayText: ""
  property color fillTextColor: Color.background
  property real labelWidth: 0

  spacing: Spacing.sm

  Column {
    Layout.preferredWidth: row.labelWidth
    Layout.alignment: Qt.AlignVCenter

    Text {
      width: parent.width
      elide: Text.ElideRight
      text: row.title
      color: Color.text
      font: Typography.display({
                                 "size": Typography.sm
                               })
    }
    Text {
      width: parent.width
      elide: Text.ElideRight
      text: row.subtitle
      color: Color.muted
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
  }
  PerformanceBar {
    Layout.fillWidth: true
    Layout.alignment: Qt.AlignVCenter
    fills: row.fills
    fillEnd: row.fillEnd
    overlayText: row.overlayText
    fillTextColor: row.fillTextColor
  }
}
