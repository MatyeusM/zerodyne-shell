import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import qs.style

// Toast stack: fullscreen transparent overlay on the bar's screen, so
// toasts always appear where the bar is (default monitor when no screens
// resolve). Bottom-right column, newest at the bottom growing upward;
// each card slides up from below on entry. Overlay layer, no keyboard
// focus, click-through everywhere except the cards themselves.
PanelWindow {
  id: root

  property var store: null
  property var anchorScreen: null
  readonly property int toastCount: root.store ? root.store.popupModel.count : 0

  visible: root.toastCount > 0
  anchors.top: true
  anchors.bottom: true
  anchors.left: true
  anchors.right: true
  color: "transparent"
  exclusionMode: ExclusionMode.Ignore
  screen: root.anchorScreen
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

  mask: Region {
    item: toastColumn
  }

  Column {
    id: toastColumn

    anchors.right: parent.right
    anchors.bottom: parent.bottom
    anchors.rightMargin: Spacing.md
    anchors.bottomMargin: Spacing.md
    spacing: Spacing.sm

    Repeater {
      model: root.store ? root.store.popupModel : null

      delegate: Item {
        id: slot

        required property int index
        required property string app
        required property string summary
        required property string body
        required property int urgency
        required property double expireTimeout
        required property bool hasDefault
        readonly property real lifetime: root.store.durationFor(slot.urgency, slot.expireTimeout)
        property real remaining: 1.0
        readonly property bool ticking: slot.lifetime > 0 && !card.hovered

        width: Sizing.sm
        height: card.implicitHeight

        onSummaryChanged: slot.remaining = 1.0
        onBodyChanged: slot.remaining = 1.0

        Timer {
          interval: 50
          repeat: true
          running: slot.ticking

          onTriggered: {
            if (slot.lifetime <= 0) {
              return
            }
            slot.remaining -= 50.0 / slot.lifetime
            if (slot.remaining <= 0) {
              slot.remaining = 0
              root.store.expireAt(slot.index)
            }
          }
        }
        NotificationCard {
          id: card

          property bool shown: false

          width: slot.width
          // Slide up from below: y starts one slot height down, eases to
          // place. Own height only, so no childrenRect sizing loop.
          y: card.shown ? 0 : slot.height + Spacing.sm
          opacity: card.shown ? 1 : 0
          app: slot.app
          summary: slot.summary
          body: slot.body
          urgency: slot.urgency
          hasDefault: slot.hasDefault

          Behavior on y {
            NumberAnimation {
              duration: 250
              easing.type: Easing.OutCubic
            }
          }
          Behavior on opacity {
            NumberAnimation {
              duration: 200
            }
          }

          Component.onCompleted: card.shown = true
          onCloseRequested: root.store.dismissAt(slot.index)
          onDefaultRequested: root.store.defaultAt(slot.index)
        }
      }
    }
  }
}
