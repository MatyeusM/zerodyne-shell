import QtQuick
import Quickshell
import Quickshell.Services.Notifications

// Notification server: takes over the freedesktop bus name and feeds the
// toast stack. Live Notification objects stay in a JS map by id (a QObject
// stored in a model role would dangle when the server destroys it); the
// ListModel carries plain snapshots only, so rows are always safe to read.
Item {
  id: root

  property alias popupModel: popupModel
  readonly property int lowDuration: 5000
  readonly property int normalDuration: 8000
  readonly property int maxDuration: 30000
  property var liveRefs: ({})

  // Sticky (0) criticals never decay; anything else gets the sender's
  // expireTimeout clamped to maxDuration, else the urgency default.
  function durationFor(urgency, expireTimeout) {
    if (urgency === NotificationUrgency.Critical) {
      return 0
    }
    var base = root.normalDuration
    if (urgency === NotificationUrgency.Low) {
      base = root.lowDuration
    }
    var asked = Number(expireTimeout || 0)
    if (isFinite(asked) && asked > 0) {
      return Math.min(root.maxDuration, asked)
    }
    return base
  }
  // Dot-assignment (not an object literal): quoted-key literals crash
  // qmlformat under this repo's .qmlformat.ini combo.
  function snapshotOf(n) {
    var snap = {}
    snap.id = n.id
    snap.app = n.appName || ""
    snap.summary = String(n.summary || "")
    snap.body = String(n.body || "")
    snap.urgency = n.urgency
    snap.expireTimeout = Number(n.expireTimeout || 0)
    snap.hasDefault = root.hasDefaultAction(n)
    snap.timestamp = Date.now()
    return snap
  }
  function hasDefaultAction(n) {
    var actions = null
    try {
      actions = n.actions
    } catch (e) {
      return false
    }
    if (!actions) {
      return false
    }
    for (var i = 0; i < actions.length; i++) {
      if (actions[i] && actions[i].identifier === "default") {
        return true
      }
    }
    return false
  }
  function indexOf(id) {
    for (var i = 0; i < popupModel.count; i++) {
      var row = popupModel.get(i)
      if (row && row.id === id) {
        return i
      }
    }
    return -1
  }
  function handleNotification(n) {
    // Tracked so the object outlives this handler; released on close.
    n.tracked = true
    var snap = root.snapshotOf(n)
    root.liveRefs[snap.id] = n
    n.closed.connect(function () {
      root.removeById(snap.id)
    })
    n.summaryChanged.connect(function () {
      root.refreshRow(snap.id)
    })
    n.bodyChanged.connect(function () {
      root.refreshRow(snap.id)
    })
    // Same id already on screen: the sender replaced it, so the old row
    // goes and the fresh content stacks at the bottom.
    root.removeById(snap.id)
    popupModel.append(snap)
  }
  // Re-copy live text into the row (replaces_id updates land on the held
  // object with no second onNotification).
  function refreshRow(id) {
    var ref = root.liveRefs[id]
    if (!ref) {
      return
    }
    var i = root.indexOf(id)
    if (i < 0) {
      return
    }
    try {
      popupModel.setProperty(i, "summary", String(ref.summary || ""))
      popupModel.setProperty(i, "body", String(ref.body || ""))
    } catch (e) {}
  }
  function takeRef(id) {
    var ref = root.liveRefs[id] || null
    if (ref) {
      delete root.liveRefs[id]
    }
    return ref
  }
  function dismissAt(i) {
    if (i < 0 || i >= popupModel.count) {
      return
    }
    var row = popupModel.get(i)
    var ref = row ? root.takeRef(row.id) : null
    popupModel.remove(i)
    if (ref) {
      try {
        ref.dismiss()
      } catch (e) {}
    }
  }
  function expireAt(i) {
    if (i < 0 || i >= popupModel.count) {
      return
    }
    var row = popupModel.get(i)
    var ref = row ? root.takeRef(row.id) : null
    popupModel.remove(i)
    if (ref) {
      try {
        ref.expire()
      } catch (e) {}
    }
  }
  // Click behavior: fire the sender's default action when it has one,
  // then take the toast off screen either way.
  function defaultAt(i) {
    if (i < 0 || i >= popupModel.count) {
      return
    }
    var row = popupModel.get(i)
    var ref = row ? root.liveRefs[row.id] || null : null
    if (ref && ref.actions) {
      try {
        for (var a = 0; a < ref.actions.length; a++) {
          var action = ref.actions[a]
          if (action && action.identifier === "default") {
            action.invoke()
            break
          }
        }
      } catch (e) {}
    }
    root.dismissAt(i)
  }
  function removeById(id) {
    var i = root.indexOf(id)
    if (i < 0) {
      return
    }
    root.takeRef(id)
    popupModel.remove(i)
  }

  ListModel {
    id: popupModel
  }
  NotificationServer {
    id: server

    keepOnReload: false
    persistenceSupported: false
    bodySupported: true
    bodyMarkupSupported: true
    actionsSupported: true
    imageSupported: false

    onNotification: notification => root.handleNotification(notification)
  }
}
