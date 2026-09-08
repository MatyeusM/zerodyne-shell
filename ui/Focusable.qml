import QtQuick

// Reusable keyboard-focus behavior and state.
//
// Uses the standard Qt focus system only; no custom focus or navigation
// framework is introduced. Place inside a widget and bind visuals to
// `focused`:
//
//   Focusable {
//     id: focusState
//     focusOnTab: true
//   }
//   Border { visible: focusState.focused; ... }
//
Item {
  id: root

  // When true, the item takes focus on creation.
  property bool autoFocus: false
  // When true, the item participates in Tab/Shift+Tab traversal.
  property bool focusOnTab: true

  // Focus state. Bind visual focus indicators to this.
  readonly property bool focused: root.activeFocus

  function requestFocus(): void {
  root.forceActiveFocus(Qt.OtherFocusReason)
}

  focus: root.autoFocus
  activeFocusOnTab: root.focusOnTab
}
