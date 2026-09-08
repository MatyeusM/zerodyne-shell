import QtQuick
import qs.style

// Link statistics as a label/value table: two pairs per row. Pure display
// over a `zerodyne status network` payload; formatting helpers are local.
// Always mounted with "--" placeholders so values popping in never shift
// the layout.
Column {
  id: root

  property var link: null

  function formatRate(bps) {
    if (bps === undefined || bps === null)
      return "--"
    if (bps >= 1000000)
      return Math.round(bps / 100000) / 10 + " Mb/s"
    if (bps >= 1000)
      return Math.round(bps / 100) / 10 + " Kb/s"
    return Math.round(bps) + " b/s"
  }
  function formatBytes(bytes) {
    if (bytes === undefined || bytes === null)
      return "--"
    if (bytes >= 1073741824)
      return Math.round(bytes / 107374182.4) / 10 + " GB"
    if (bytes >= 1048576)
      return Math.round(bytes / 104857.6) / 10 + " MB"
    if (bytes >= 1024)
      return Math.round(bytes / 102.4) / 10 + " KB"
    return Math.round(bytes) + " B"
  }
  function formatMs(value) {
    if (value === undefined || value === null)
      return "--"
    return Math.round(value * 10) / 10 + " ms"
  }
  function num(value) {
    return value === undefined || value === null ? "--" : value
  }

  spacing: Spacing.xs
  visible: root.link !== null

  TextMetrics {
    id: labelMeasure

    text: "Downloaded"
    font: Typography.display({
                               "size": Typography.xs
                             })
  }
  Text {
    text: "Statistics"
    color: Color.muted
    font: Typography.display({
                               "size": Typography.sm
                             })
  }
  Grid {
    columns: 4
    columnSpacing: Spacing.md
    rowSpacing: Spacing.xxs

    Text {
      width: labelMeasure.width
      text: "Ping"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.formatMs(root.link && root.link.internet_ping_avg_ms)
      color: Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
    Text {
      width: labelMeasure.width
      text: "Packet Loss"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.link && root.link.internet_loss_pct !== undefined ? root.link.internet_loss_pct
                                                                     + "%" : "--"
      color: root.link && root.link.internet_loss_pct > 0 ? Color.tertiary : Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
    Text {
      width: labelMeasure.width
      text: "Receiving"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.formatRate(root.link && root.link.download_bps)
      color: Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
    Text {
      width: labelMeasure.width
      text: "Sending"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.formatRate(root.link && root.link.upload_bps)
      color: Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
    Text {
      width: labelMeasure.width
      text: "Downloaded"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.formatBytes(root.link && root.link.rx_bytes)
      color: Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
    Text {
      width: labelMeasure.width
      text: "Uploaded"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.formatBytes(root.link && root.link.tx_bytes)
      color: Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
    Text {
      width: labelMeasure.width
      text: "IP Address"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.num(root.link && root.link.ip)
      color: Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
    Text {
      width: labelMeasure.width
      text: "Gateway"
      color: Color.muted
      font: Typography.display({
                                 "size": Typography.xs
                               })
    }
    Text {
      text: root.num(root.link && root.link.gateway)
      color: Color.text
      font: Typography.mono({
                              "size": Typography.xs
                            })
    }
  }
}
