pragma Singleton
import QtQuick
import Quickshell
import qs.utility

Singleton {
  property color background: "#020307"
  property color surface: "#090b12"
  property color altSurface: "#13161d"
  property color primary: "#2bfdf4"
  property color secondary: "#e7e824"
  property color tertiary: "#9c113f"
  property color text: "#dadee8"
  property color muted: "#8b8f99"

  function alpha(color, opacity) {
    var a = Numbers.clamp(opacity, 0, 1)
    return Qt.rgba(color.r, color.g, color.b, a)
  }
}
