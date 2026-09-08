import QtQuick
import Quickshell.Hyprland
import qs.style
import qs.ui

// Left-side Hyprland workspace switcher. Shows the active workspaces plus
// stable numbered slots up to monitors x 2, so the indicator stays compact
// while offering navigation destinations.
//
// A slot is disabled when its workspace is already active on another
// monitor: switching to it would steal the workspace instead of navigating.
Item {
  id: root

  readonly property var monitorList: Hyprland.monitors.values
  readonly property int slotCount: Math.max(1, root.monitorList.length) * 2

  // Sorted union of stable slots, existing workspaces, and per-monitor
  // active workspaces.
  readonly property var shownIds: {
    var seen = {}
    var i
    var existing = Hyprland.workspaces.values
    for (i = 0; i < existing.length; i++) {
      if (existing[i].id > 0)
        seen[existing[i].id] = true
    }
    var mons = root.monitorList
    for (i = 0; i < mons.length; i++) {
      var aws = mons[i].activeWorkspace
      if (aws !== null && aws !== undefined && aws.id > 0)
        seen[aws.id] = true
    }
    var ids = []
    for (i = 1; i <= root.slotCount; i++)
      ids.push(i)
    for (var key in seen) {
      var id = parseInt(key, 10)
      if (ids.indexOf(id) === -1)
        ids.push(id)
    }
    ids.sort(function (a, b) {
      return a - b
    })
    return ids
  }

  // Workspace id active on the focused monitor (-1 when unknown).
  readonly property int activeHereId: {
    var mon = Hyprland.focusedMonitor
    if (mon !== null && mon !== undefined) {
      var aws = mon.activeWorkspace
      if (aws !== null && aws !== undefined)
        return aws.id
    }
    var focused = Hyprland.focusedWorkspace
    return focused !== null && focused !== undefined ? focused.id : -1
  }

  function workspaceById(id) {
    var values = Hyprland.workspaces.values
    for (var i = 0; i < values.length; i++) {
      if (values[i].id === id)
        return values[i]
    }
    return null
  }
  function goTo(id) {
    var ws = root.workspaceById(id)
    if (ws !== null) {
      ws.activate()
      return
    }
    // No workspace object exists for empty slots. Hyprland.dispatch
    // interpolates its argument as raw Lua (quickshell 0.3.x bridge), so
    // the workspace dispatcher goes through hl.dsp.focus.
    Hyprland.dispatch("hl.dsp.focus({workspace = \"" + id + "\"})")
  }

  // Bar-module contract: full bar height. Width follows the row.
  implicitWidth: buttons.implicitWidth + Spacing.sm * 2
  implicitHeight: Spacing.lg

  Container {
    anchors.fill: parent
    background: Color.surface
    cornerStyle: Container.Chamfered
    chamferBottomRight: true
    chamferSize: Spacing.sm
    // Small uniform inset for button height; the visible left inset stays
    // Spacing.sm via the row margin below.
    padding: Spacing.xxs

    Row {
      id: buttons

      // Fills the content box exactly: fixed position and (in the bar)
      // fixed height, so childrenRect sizing stays loop-free. Buttons are
      // square: explicit width follows the row height.
      anchors.fill: parent
      anchors.leftMargin: Spacing.xxs
      spacing: 0

      Repeater {
        model: root.shownIds

        WorkspaceButton {
          required property int modelData

          height: buttons.height
          // Slightly wider than tall: 3:2 ratio off the row height.
          width: height * 1.5
          label: String(modelData)
          active: modelData === root.activeHereId
          disabled: {
            var ws = root.workspaceById(modelData)
            return ws !== null && ws.active && modelData !== root.activeHereId
          }

          onActivated: root.goTo(modelData)
        }
      }
    }
  }
  Border {
    anchors.fill: parent
    showTop: false
    showLeft: false
    borderWidth: 2
    borderColor: Color.primary
    cornerStyle: Border.Chamfered
    chamferBottomRight: true
    chamferSize: Spacing.sm
  }
}
