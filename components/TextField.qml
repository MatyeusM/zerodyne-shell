import QtQuick
import qs.style
import qs.ui

// Minimal single-line text input surface built on QtQuick TextInput.
//
// A HoverHandler (not Clickable) senses hover so the TextInput keeps its
// own press/focus handling and typing is never intercepted.
//
//   TextField {
//     placeholderText: "Search"
//     onAccepted: root.submit(text)
//   }
//
Item {
  id: root

  // Aliased to the inner TextInput; consumer bindings stay two-way.
  property alias text: input.text
  property string placeholderText: ""
  property bool disabled: false
  property int echoMode: TextInput.Normal
  property int maximumLength: 32767
  property real fieldPadding: Spacing.sm
  property real radius: 0
  property color background: Theme.altSurface
  property color hoverBackground: Theme.surface
  property color textColor: Theme.text
  property color placeholderColor: Theme.muted
  property color disabledTextColor: Theme.muted
  readonly property alias hovered: hover.hovered
  readonly property alias focused: input.activeFocus

  signal accepted
  signal editingFinished

  function forceFocus(): void {
  input.forceActiveFocus(Qt.OtherFocusReason)
}

  implicitWidth: 160
  implicitHeight: input.implicitHeight + root.fieldPadding * 2

  Disabled {
    id: disabledState

    disabled: root.disabled
  }
  HoverBackground {
    id: hoverBg

    hovered: hover.hovered
    normalBackground: root.background
    hoverBackground: root.hoverBackground
  }
  Container {
    anchors.fill: parent
    background: hoverBg.current
    padding: root.fieldPadding
    radius: root.radius

    TextInput {
      id: input

      anchors.fill: parent
      verticalAlignment: TextInput.AlignVCenter
      clip: true
      color: root.disabled ? root.disabledTextColor : root.textColor
      selectionColor: Theme.primary
      selectedTextColor: Theme.background
      cursorVisible: focused && !root.disabled
      readOnly: disabledState.disabled
      echoMode: root.echoMode
      maximumLength: root.maximumLength
      font: Typography.mono({
                              "size": Typography.sm
                            })

      onAccepted: root.accepted()
      onEditingFinished: root.editingFinished()
    }
    Text {
      anchors.fill: input
      verticalAlignment: Text.AlignVCenter
      clip: true
      text: root.placeholderText
      color: root.placeholderColor
      font: input.font
      visible: input.text === "" && root.placeholderText !== ""
    }
  }
  HoverHandler {
    id: hover

    cursorShape: Qt.IBeamCursor
  }
}
