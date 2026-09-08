import QtQuick
import QtQuick.Controls
import qs.components
import qs.style

// Visible wifi network list: header plus scrollable rows. Display plus
// user intent; the panel owns all zerodyne calls and tells the rows when
// to prompt.
Column {
  id: root

  property var networks: []
  property string promptFor: ""
  property bool busy: false
  property bool scanning: false

  signal connectRequested(ssid: string, password: string)
  signal cancelPrompt
  signal forgetRequested(ssid: string)

  spacing: Spacing.sm

  Text {
    text: root.scanning ? "Wi-Fi networks · scanning…" : "Wi-Fi networks"
    color: Color.muted
    font: Typography.display({
                               "size": Typography.sm
                             })
  }
  ListView {
    width: parent.width
    height: Math.min(contentHeight, Spacing.xxl * 3)
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: root.networks

    ScrollBar.vertical: AutoScrollbar {
      id: wifiBar
    }
    delegate: WifiRow {
      required property var modelData

      width: ListView.view.width - (ListView.view.contentHeight > ListView.view.height ? wifiBar.gutter :
                                                                                         0)
      ssid: modelData.ssid
      signal: modelData.signal
      security: modelData.security
      inUse: modelData.in_use
      known: modelData.known
      prompt: modelData.ssid === root.promptFor
      busy: root.busy

      onConnectRequested: (ssid, password) => root.connectRequested(ssid, password)
      onCancelPrompt: root.cancelPrompt()
      onForgetRequested: ssid => root.forgetRequested(ssid)
    }
  }
}
