import QtQuick
import qs.style
import qs.ui

// Single workspace number. Composes the interaction primitives; no local
// mouse handling and no workspace knowledge beyond its props.
//
// States: active (altSurface + primary text, fully non-interactive),
// hover (secondary text), disabled (muted, shown but not clickable).
Item {
  id: root

  property string label: ""
  // Active on the focused monitor: shown, never interactive.
  property bool active: false
  // Active on another monitor: shown but not clickable.
  property bool disabled: false
  property real horizontalPadding: Spacing.sm
  readonly property alias hovered: click.hovered

  signal activated

  implicitWidth: labelItem.implicitWidth + root.horizontalPadding * 2
  implicitHeight: labelItem.implicitHeight

  Disabled {
    id: disabledState

    disabled: root.disabled
  }
  HoverColor {
    id: hoverFg

    hovered: click.hovered && !root.active
    normalColor: root.active ? Color.primary : Color.text
    hoverColor: Color.secondary
  }
  Container {
    anchors.fill: parent
    background: root.active ? Color.altSurface : "transparent"

    Text {
      id: labelItem

      // Fill-aligned (not centerIn): keeps childrenRect sizing loop-free.
      anchors.fill: parent
      leftPadding: root.horizontalPadding
      rightPadding: root.horizontalPadding
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      text: root.label
      color: root.disabled ? Color.muted : hoverFg.current
      font: Typography.display()
    }
  }
  Clickable {
    id: click

    anchors.fill: parent
    enabled: disabledState.enabled && !root.active
    cursorShape: root.active ? Qt.ArrowCursor : Qt.PointingHandCursor

    onClicked: root.activated()
  }
}
