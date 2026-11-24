import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pam
import Quickshell.Wayland
import qs.style

// Session lock host. Quickshell creates one WlSessionLockSurface per output:
// only the DP-2 surface hosts the password view, every other output stays on
// the dark background. Authentication uses Quickshell's in-process PamContext
// (same as omarchy) — no scripts, and the password never reaches a subprocess
// or the log. `lock unlock` is kept as a recovery hatch from a TTY.
Item {
  id: root

  readonly property string userName: Quickshell.env("USER") || Quickshell.env("LOGNAME")
  property bool pamConfigured: false
  property bool authenticating: false
  property string pendingPassword: ""
  property string failureMessage: ""
  property int failedAttempts: 0

  function isLocked(): bool {
    return sessionLock.locked
  }
  // The password field lives on DP-2 only. If that output is missing, fall
  // back to the first screen so the session is never left without a way in.
  function targetScreenName(): string {
    var ss = Quickshell.screens
    for (var i = 0; i < ss.length; i++) {
      if (ss[i].name === "DP-2")
        return "DP-2"
    }
    return ss.length > 0 ? ss[0].name : ""
  }
  function resetAuth() {
    authenticating = false
    pendingPassword = ""
    failureMessage = ""
    failedAttempts = 0
  }
  function lock(): bool {
    if (sessionLock.locked)
      return false
    resetAuth()
    sessionLock.locked = true
    return true
  }
  function unlock() {
    // Recovery hatch callable from a TTY if PAM is misconfigured. The UI
    // itself has no unauthenticated way out.
    sessionLock.locked = false
    resetAuth()
  }
  function submitPassword(value) {
    if (!pamConfigured) {
      failureMessage = "No PAM stack configured"
      return
    }
    if (authenticating || sessionLock.locked === false || value.length === 0)
      return
    pendingPassword = value
    failureMessage = ""
    authenticating = true
    if (!passwordPam.start())
      handleFailure()
    else
      Qt.callLater(respondToPam)
  }
  function respondToPam() {
    if (authenticating && passwordPam.active && passwordPam.responseRequired)
      passwordPam.respond(pendingPassword)
  }
  function handleFailure() {
    if (!authenticating)
      return
    authenticating = false
    pendingPassword = ""
    failedAttempts += 1
    failureMessage = "Authentication failed (" + failedAttempts + ")"
  }
  function finishUnlock() {
    sessionLock.locked = false
    resetAuth()
    console.log("lock: unlocked by PAM")
  }

  WlSessionLock {
    id: sessionLock

    locked: false

    onLockStateChanged: {
      console.log("lock: session locked =", sessionLock.locked)
    }

    WlSessionLockSurface {
      id: lockSurface

      color: Theme.background

      // Only the designated surface instantiates the field, so secondary
      // outputs cannot steal keyboard focus from it.
      Loader {
        active: lockSurface.screen !== null && lockSurface.screen.name === root.targetScreenName()
        anchors.fill: parent

        sourceComponent: Component {
          LockView {
            anchors.fill: parent
            failureMessage: root.failureMessage
            authenticating: root.authenticating

            onSubmitPassword: function (password) {
              root.submitPassword(password)
            }
          }
        }
      }
    }
  }
  PamContext {
    id: passwordPam

    // System local-login stack: pam_unix via system-auth. No hyprlock
    // dependency. Swap in a dedicated /etc/pam.d stack if one is added.
    config: "system-local-login"
    user: root.userName

    onResponseRequiredChanged: root.respondToPam()
    onPamMessage: root.respondToPam()
    onCompleted: function (result) {
      if (!root.authenticating)
        return
      if (result === PamResult.Success)
        root.finishUnlock()
      else
        root.handleFailure()
    }
    onError: function (error) {
      root.handleFailure()
    }
  }
  FileView {
    path: "/etc/pam.d/system-local-login"
    watchChanges: true
    printErrors: false

    onLoaded: root.pamConfigured = true
    onLoadFailed: root.pamConfigured = false
    onFileChanged: reload()
  }
  IpcHandler {
    function lock(): string {
      return root.lock() ? "ok" : "already-locked"
    }
    function unlock(): string {
      root.unlock()
      return "ok"
    }
    function status(): string {
      return root.isLocked() ? "locked" : "unlocked"
    }

    target: "lock"
  }
}
