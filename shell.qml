import Quickshell
import Quickshell.Hyprland
import "modules/bar"
import "modules/launcher"
import "modules/notifications"

ShellRoot {
  id: shell

  // Screen holding Hyprland's focused monitor: the launcher opens where
  // the user is looking, not where the bar lives. Falls back to the bar
  // screen (own child: safe declare-before-use target), then the first
  // screen, so the dialog always has somewhere to open.
  readonly property var activeScreen: {
    var fm = Hyprland.focusedMonitor
    var ss = Quickshell.screens
    if (fm) {
      for (var i = 0; i < ss.length; i++) {
        if (ss[i].name === fm.name) {
          return ss[i]
        }
      }
    }
    if (bar.barScreen !== null && bar.barScreen !== undefined) {
      return bar.barScreen
    }
    return ss.length > 0 ? ss[0] : null
  }

  Bar {
    id: bar
  }
  Notifications {
    id: notifs
  }
  // Toasts slave to the bar window's screen (same object, not a copy), so
  // they migrate with the bar across fullscreen transitions by
  // construction. Verified: overlay and bar share one layer-shell
  // output in steady state and across focus changes.
  NotificationToasts {
    store: notifs
    anchorScreen: bar.barScreen
  }
  LauncherWindow {
    dialogScreen: shell.activeScreen
  }
}
