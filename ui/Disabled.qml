// Reusable disabled-state behavior.
// Provides the single `disabled` flag widgets use to represent disabled
// state. No visual appearance is assumed here; each widget decides how to
// render itself when disabled (muted colors, ignored input, or both):
//   Disabled {
//     id: disabledState
//     disabled: root.disabled
//   }
//   Clickable { enabled: disabledState.enabled; ... }

import QtQuick

QtObject {
  id: root

  property bool disabled: false
  // Convenience inverse for binding to `enabled` properties.
  readonly property bool enabled: !root.disabled
}
