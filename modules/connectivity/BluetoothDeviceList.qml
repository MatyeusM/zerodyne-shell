import QtQuick
import QtQuick.Controls
import qs.components
import qs.style

// Bluetooth device sections: connected devices, remembered devices, and
// scan results with PAIRED/AVAILABLE headers. Display plus user intent
// the panel owns grouping input, pending state, and all zerodyne calls.
// Rows carry primitives only, never BlueZ wrappers.
Column {
  id: root

  property var devices: []
  property var pendingActions: ({})
  property string emptyText: "No devices found"

  signal activateRow(address: string, connected: bool, known: bool)
  signal forgetRow(address: string)

  // Primitives-only projection of one device for list rows.
  function deviceRow(d) {
    if (!d)
      return null
    var row = {}
    row.address = d.address || ""
    row.name = deviceLabel(d)
    row.connected = !!d.connected
    row.known = !!(d.paired || d.bonded || d.trusted)
    row.detail = rowDetail(d)
    row.icon = d.icon || ""
    return row
  }
  function deviceLabel(d) {
    if (!d)
      return ""
    return String(d.deviceName || d.name || "").trim()
  }
  function isUuidLike(value) {
    var text = String(value || "").trim()
    if (text === "")
      return false
    return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(text) || /^[0-9a-f]{32}$/i.test(text) || /^0x[0-9a-f]{4,32}$/i.test(text) || /^0000[0-9a-f]{4}-0000-1000-8000-00805f9b34fb$/i.test(text)
  }
  function isAddressLike(value) {
    return /^([0-9a-f]{2}[:-]){5}[0-9a-f]{2}$/i.test(String(value || "").trim())
  }
  function hasHumanName(d) {
    var label = deviceLabel(d)
    return label !== "" && !isUuidLike(label) && !isAddressLike(label)
  }
  function rowDetail(d) {
    if (d && d.connected && d.batteryAvailable)
      return Math.round(d.battery * 100) + "%"
    return ""
  }
  function sortedRows(rows) {
    return rows.sort(function (a, b) {
      return a.name.localeCompare(b.name)
    })
  }
  // Grouped rows: connected, remembered (paired/bonded/trusted), and
  // unremembered devices visible while scanning.
  function groupDevices(devices) {
    var connected = []
    var known = []
    var discovered = []
    var values = devices || []
    for (var i = 0; i < values.length; i++) {
      var d = values[i]
      if (!d || !hasHumanName(d))
        continue
      var row = deviceRow(d)
      if (d.connected)
        connected.push(row)
      else if (d.paired || d.bonded || d.trusted)
        known.push(row)
      else
        discovered.push(row)
    }
    var groups = {}
    groups.connected = sortedRows(connected)
    groups.known = sortedRows(known)
    groups.discovered = sortedRows(discovered)
    return groups
  }
  // Flat known-plus-scan model with section tags for one ListView.
  function flatRows(groups) {
    var rows = []
    var known = groups.known || []
    for (var k = 0; k < known.length; k++) {
      var knownRow = {}
      knownRow.dev = known[k]
      knownRow.section = "known"
      rows.push(knownRow)
    }
    var found = groups.discovered || []
    for (var s = 0; s < found.length; s++) {
      var foundRow = {}
      foundRow.dev = found[s]
      foundRow.section = "discovered"
      rows.push(foundRow)
    }
    return rows
  }
  // A row opens a section when it is the first of its kind.
  function sectionTitle(index) {
    var rows = root.scrollRows
    if (index < 0 || index >= rows.length)
      return ""
    if (index > 0 && rows[index - 1].section === rows[index].section)
      return ""
    return rows[index].section === "known" ? "PAIRED" : "AVAILABLE"
  }
  function pendingText(action) {
    if (action === "connecting")
      return "Connecting…"
    if (action === "disconnecting")
      return "Disconnecting…"
    if (action === "forgetting")
      return "Forgetting…"
    return ""
  }
  function pendingFor(address) {
    var action = address && root.pendingActions[address] ? root.pendingActions[address] : ""
    return root.pendingText(action)
  }

  readonly property var groups: root.groupDevices(root.devices)
  readonly property var connectedRows: root.groups.connected || []
  readonly property var scrollRows: root.flatRows(root.groups)

  spacing: Spacing.sm

  Separator {
    visible: root.connectedRows.length > 0
  }
  Column {
    visible: root.connectedRows.length > 0
    width: parent.width
    spacing: Spacing.sm

    Text {
      text: "CONNECTED"
      color: Color.muted
      font: Typography.display({
        "size": Typography.sm
      })
    }
    Repeater {
      model: root.connectedRows

      BluetoothRow {
        required property var modelData

        width: parent.width
        address: modelData.address
        name: modelData.name
        connected: true
        known: modelData.known
        pending: root.pendingFor(modelData.address)
        detail: modelData.detail
        iconName: modelData.icon

        onActivated: root.activateRow(modelData.address, true, modelData.known)
        onForgetRequested: address => root.forgetRow(address)
      }
    }
  }
  Separator {
    visible: root.connectedRows.length > 0 && root.scrollRows.length > 0
  }
  ListView {
    width: parent.width
    height: Math.min(contentHeight, Spacing.xxl * 3)
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: root.scrollRows

    ScrollBar.vertical: AutoScrollbar {
      id: deviceBar
    }
    delegate: Column {
      required property var modelData
      required property int index

      width: ListView.view.width - (ListView.view.contentHeight > ListView.view.height ? deviceBar.gutter :
                                                                                         0)
      spacing: 0

      Text {
        // No height binding: the Column collapses invisible children
        // on its own; binding height to implicitHeight loops.
        visible: root.sectionTitle(index) !== ""
        text: root.sectionTitle(index)
        color: Color.muted
        font: Typography.display({
          "size": Typography.sm
        })
      }
      BluetoothRow {
        width: parent.width
        address: modelData.dev.address
        name: modelData.dev.name
        connected: false
        known: modelData.dev.known
        pending: root.pendingFor(modelData.dev.address)
        detail: modelData.dev.detail
        iconName: modelData.dev.icon

        onActivated: root.activateRow(modelData.dev.address, false, modelData.dev.known)
        onForgetRequested: address => root.forgetRow(address)
      }
    }
  }
  Text {
    visible: root.connectedRows.length === 0 && root.scrollRows.length === 0
    width: parent.width
    wrapMode: Text.Wrap
    text: root.emptyText
    color: Color.muted
    font: Typography.display({
      "size": Typography.sm
    })
  }
}
