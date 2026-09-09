import Quickshell
import "modules/bar"
import "modules/launcher"
import "modules/notifications"

ShellRoot {
  id: shell

  Bar {
    id: bar
  }
  Notifications {
    id: notifs
  }
  NotificationToasts {
    store: notifs
    anchorScreen: bar.barScreen
  }
  LauncherWindow {
    dialogScreen: bar.barScreen
  }
}
