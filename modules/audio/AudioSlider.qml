import QtQuick
import qs.style

// Dumb track slider for the audio panel (stays in modules/audio until a
// second consumer exists). The panel owns all zerodyne calls; this only
// reports intent. Right-click reports a mute toggle (omarchy PanelSlider
// onRightClicked precedent).
//
//   AudioSlider {
//     value: root.volumePct
//     onMoved: v => root.setOutputVolume(v)
//     onMuteRequested: root.toggleOutputMute()
//   }
Item {
  id: root

  property real value: 0
  property real minimum: 0
  property real maximum: 100
  property real step: 5
  property bool dragging: false
  // Live position while dragging; the track follows this so feedback
  // never waits on the re-poll.
  property real liveValue: root.value
  readonly property real fraction: root.maximum > root.minimum ? Math.max(0, Math.min(1, ((
                                                                                            root.dragging
                                                                                            ? root.liveValue :
                                                                                              root.value)
                                                                                          - root.minimum)
                                                                                      / (root.maximum
                                                                                         - root.minimum))) :
                                                                 0

  signal moved(v: real)
  signal muteRequested

  function setFromX(x) {
    var v = root.minimum + Math.max(0, Math.min(1, x / root.width)) * (root.maximum - root.minimum)
    v = Math.round(v / root.step) * root.step
    root.liveValue = v
    root.moved(v)
  }

  implicitHeight: Spacing.md

  Rectangle {
    id: track

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    height: Spacing.xxs
    color: Color.muted
  }
  Rectangle {
    anchors.left: track.left
    anchors.verticalCenter: track.verticalCenter
    width: track.width * root.fraction
    height: track.height
    color: Color.text
  }
  Rectangle {
    anchors.verticalCenter: track.verticalCenter
    x: track.width * root.fraction - width / 2
    width: Spacing.sm
    height: Spacing.sm
    radius: width / 2
    color: Color.text
  }
  MouseArea {
    anchors.fill: parent
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    cursorShape: Qt.PointingHandCursor

    onPressed: mouse => {
      if (mouse.button === Qt.RightButton) {
        root.muteRequested()
        return
      }
      root.dragging = true
      root.setFromX(mouse.x)
    }
    onPositionChanged: mouse => {
      if (root.dragging)
        root.setFromX(mouse.x)
    }
    onReleased: root.dragging = false
  }
}
