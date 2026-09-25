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

// Top bar: layout and composition only. Widget logic lives in the sibling
// modules. Transparent: each module paints its own background, the gaps
// between them show the desktop.
//
// The bar lives on the first screen by default. When the monitor it is on
// shows a real-fullscreen app, it moves to the next monitor without one.
// Maximized windows keep the bar and never trigger a move.
PanelWindow {
  id: root

  // Logical bar screen: home (first screen) unless it shows a
  // real-fullscreen app, else the first non-fullscreen screen. Maximized
  // windows keep their gaps and the bar, so they never trigger a move.
  // Null when every screen is fullscreen, so the bar hides instead of
  // jumping back onto a fullscreen monitor and kicking it out of
  // fullscreen.
  readonly property var barScreen: {
    var screens = Quickshell.screens
    if (screens.length === 0)
      return null
    if (!root.realFullscreenOnScreen(screens[0].name))
      return screens[0]
    for (var j = 1; j < screens.length; j++) {
      if (!root.realFullscreenOnScreen(screens[j].name))
        return screens[j]
    }
    return null
  }

  // True when the Hyprland monitor behind the given screen has a real
  // fullscreen app (client fullscreen state 2+) on its active workspace.
  // Maximized windows report state 1 and keep the bar, so they are
  // ignored. Workspace.hasFullscreen is true for both states and cannot
  // tell them apart.
  function realFullscreenOnScreen(screenName) {
    var mons = Hyprland.monitors.values
    for (var i = 0; i < mons.length; i++) {
      if (mons[i].name === screenName) {
        var ws = mons[i].activeWorkspace
        if (ws === null || ws === undefined)
          return false
        var tls = ws.toplevels.values
        for (var k = 0; k < tls.length; k++) {
          var o = tls[k].lastIpcObject
          if (o && o.fullscreen > 1)
            return true
        }
        return false
      }
    }
    return false
  }

  anchors.top: true
  anchors.left: true
  anchors.right: true
  implicitHeight: Spacing.lg
  color: "transparent"
  // Hidden when every monitor is fullscreen (barScreen null): no free
  // monitor exists, so unmapping beats covering a fullscreen window.
  // A null screen is tolerated while hidden (same pattern as
  // NotificationToasts anchorScreen).
  visible: root.barScreen !== null
  screen: root.barScreen

  // Toplevel IPC snapshots go stale across fullscreen transitions unless
  // re-fetched, so refresh them when Hyprland reports window changes.
  Connections {
    function onRawEvent(event) {
      var n = event.name
      if (n === "fullscreen" || n === "openwindow" || n === "closewindow" || n === "movewindow")
        Hyprland.refreshToplevels()
    }

    target: Hyprland
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
