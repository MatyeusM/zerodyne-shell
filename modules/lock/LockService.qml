import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pam
import Quickshell.Wayland
import qs.style

// Session lock host. Quickshell creates one WlSessionLockSurface per output.
// Every surface gets a focusable field, but only the main output (the first
// screen with the largest resolution) shows the visuals — the others stay
// dark. That way typing with the pointer on a side monitor still lands in the
// shared password state and is rendered on the main screen. Auth uses
// Quickshell's in-process PamContext — no scripts, no password in a
// subprocess or the log. `lock unlock` is a recovery hatch from a TTY.
Item {
  id: root

  readonly property string userName: Quickshell.env("USER") || Quickshell.env("LOGNAME")
  property bool pamConfigured: false
  property bool authenticating: false
  // Single source of truth shared by every surface's field.
  property string enteredPassword: ""
  property string pendingPassword: ""
  property string failureMessage: ""
  property int failedAttempts: 0

  function isLocked(): bool {
    return sessionLock.locked
  }
  // Main output = first screen with the largest pixel area. No hardcoded
  // connector name, so monitor swaps keep working.
  function mainScreen(): var {
    var ss = Quickshell.screens
    if (ss.length === 0)
      return null
    var best = ss[0]
    var bestArea = best.width * best.height
    for (var i = 1; i < ss.length; i++) {
      var area = ss[i].width * ss[i].height
      if (area > bestArea) {
        best = ss[i]
        bestArea = area
      }
    }
    return best
  }
  function isMainScreen(screen): bool {
    var main = root.mainScreen()
    return main !== null && screen !== null && main.name === screen.name
  }
  function resetAuth() {
    authenticating = false
    enteredPassword = ""
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

      // A field on every surface: whichever output the pointer is over gets
      // compositor keyboard focus and can accept typing. Only the main
      // output's instance draws the clock and dots.
      LockView {
        anchors.fill: parent
        showVisuals: lockSurface.screen !== null && root.isMainScreen(lockSurface.screen)
        passwordText: root.enteredPassword
        failureMessage: root.failureMessage
        authenticating: root.authenticating

        onPasswordEdited: function (text) {
          root.enteredPassword = text
        }
        onSubmitRequested: function (value) {
          root.submitPassword(value)
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
