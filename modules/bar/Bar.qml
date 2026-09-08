import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.modules.audio
import qs.modules.clock
import qs.modules.connectivity
import qs.modules.performance
import qs.modules.power
import qs.modules.tray
import qs.modules.workspace
import qs.style
import qs.ui

// Top bar: layout and composition only. Widget logic lives in the sibling
// modules. Uses the background color token, Spacing.lg rhythm, and a bottom
// border.
//
// The bar lives on the first screen by default. When the monitor it is on
// shows a fullscreen app, it moves to the next monitor without one.
PanelWindow {
  id: root

  // Swap to Color.muted when the design calls for it.
  property color bottomBorderColor: Color.muted
  readonly property var barScreen: {
    var screens = Quickshell.screens
    if (screens.length === 0)
      return null
    if (!root.fullscreenOnScreen(screens[0].name))
      return screens[0]
    for (var j = 1; j < screens.length; j++) {
      if (!root.fullscreenOnScreen(screens[j].name))
        return screens[j]
    }
    return screens[0]
  }

  // True when the Hyprland monitor behind the given screen has a
  // fullscreen app on its active workspace.
  function fullscreenOnScreen(screenName) {
    var mons = Hyprland.monitors.values
    for (var i = 0; i < mons.length; i++) {
      if (mons[i].name === screenName) {
        var ws = mons[i].activeWorkspace
        return ws !== null && ws !== undefined && ws.hasFullscreen
      }
    }
    return false
  }

  anchors.top: true
  anchors.left: true
  anchors.right: true
  implicitHeight: Spacing.lg
  color: Color.background
  screen: root.barScreen

  // Painted first so module borders sit on top of it where they meet.
  Border {
    anchors.fill: parent
    showTop: false
    showLeft: false
    showRight: false
    borderWidth: 2
    borderColor: root.bottomBorderColor
  }
  WorkspaceSwitcher {
    id: switcher

    anchors.left: parent.left
    anchors.top: parent.top
    anchors.bottom: parent.bottom
  }
  DateClock {
    id: clock

    // Full bar height: the clock bottom border then shares the bar bottom
    // edge and overpaints the bar border instead of floating above it.
    // Above the indicator below: the indicator slides under the clock so
    // their bottom borders join, while the clock's right edge keeps
    // dividing on top.
    z: 1
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.horizontalCenter: parent.horizontalCenter
    calendarScreen: root.barScreen
  }
  AudioIndicator {
    id: audio

    // Immediately left of the clock, tucked under it by the clock's
    // chamfer cut so both bottom borders join into one line. The clock
    // stays on top, otherwise this background would eat its left edge.
    anchors.right: clock.left
    anchors.rightMargin: -clock.chamferCut
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    panelScreen: root.barScreen
  }
  ConnectivityIndicator {
    id: connectivity

    // Immediately right of the clock, tucked under it by the clock's
    // chamfer cut so both bottom borders join into one line. The clock
    // stays on top, otherwise this background would eat its right edge.
    anchors.left: clock.right
    anchors.leftMargin: -clock.chamferCut
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    panelScreen: root.barScreen
  }
  PowerIndicator {
    id: power

    // Far right edge: full bar height so its bottom border joins the bar
    // bottom edge. The tray tucks under it by the chamfer cut so both
    // bottom borders join into one line; this item stays on top of both
    // neighbors.
    z: 2
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    panelScreen: root.barScreen
  }
  Tray {
    id: tray

    // Left of the power button, tucked under it by its chamfer cut.
    // Above the performance indicator, which tucks under this item.
    z: 1
    anchors.right: power.left
    anchors.rightMargin: -power.chamferCut
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    barScreen: root.barScreen
  }
  PerformanceIndicator {
    id: performance

    // Left of the tray, tucked under it by the tray's chamfer cut so
    // both bottom borders join into one line. The tray stays on top.
    anchors.right: tray.left
    anchors.rightMargin: -tray.chamferCut
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    panelScreen: root.barScreen
  }
}
