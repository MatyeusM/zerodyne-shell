import QtQuick
import QtQuick.Shapes
import QtQuick.VectorImage
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.services
import qs.style

// Default wallpaper: one background layer-shell surface per screen, with the
// Arch crystal centred in a square whose side is the smaller screen dimension
// divided by three. zerodyne writes a brand-coloured SVG to a runtime file and
// QtQuick.VectorImage renders it as true vector geometry (no QSvgRenderer).
Item {
  id: root

  property string sourceUrl: ""

  function hexColor(color): string {
    var r = ("0" + Math.round(color.r * 255).toString(16)).slice(-2)
    var g = ("0" + Math.round(color.g * 255).toString(16)).slice(-2)
    var b = ("0" + Math.round(color.b * 255).toString(16)).slice(-2)
    return "#" + r + g + b
  }
  function updateBrandColor() {
    colorProc.command = Zerodyne.execCommand("wallpaper-color", [root.hexColor(Theme.primary)])
    colorProc.running = true
  }

  Component.onCompleted: root.updateBrandColor()

  Process {
    id: colorProc

    stdout: StdioCollector {
      onStreamFinished: {
        var result = Zerodyne.parseOutput(text)
        if (result.ok && result.data && result.data.message)
          root.sourceUrl = "file://" + result.data.message
      }
    }
  }
  Connections {
    function onPrimaryChanged() {
      root.updateBrandColor()
    }

    target: Theme
  }
  IpcHandler {
    function update(): string {
      root.updateBrandColor()
      return "ok"
    }

    target: "wallpaper"
  }
  Variants {
    model: Quickshell.screens

    PanelWindow {
      id: wallpaperWindow

      required property var modelData

      screen: modelData
      anchors.top: true
      anchors.bottom: true
      anchors.left: true
      anchors.right: true
      color: Theme.background
      visible: true
      // The background layer can lose its buffer without this (omarchy does
      // the same for its wallpaper surfaces).
      updatesEnabled: true
      WlrLayershell.namespace: "zerodyne-wallpaper"
      WlrLayershell.layer: WlrLayer.Background
      WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
      exclusionMode: ExclusionMode.Ignore

      // Guarantees the surface is fully painted at its edges.
      Rectangle {
        anchors.fill: parent
        color: Theme.background
      }
      // Two soft brand glows. Their centres sit outside the top/bottom edges
      // (at 0.33W/-0.33H and 0.67W/1.33H), so only the fade bleeds in.
      // Declared before the crystal, which keeps it on top.
      // The gradient paths deliberately run 1px past the surface on the right
      // and bottom: Qt leaves the closing edge unpainted, which showed up as
      // a 1px white line against the compositor's background.
      Shape {
        anchors.fill: parent

        ShapePath {
          fillGradient: RadialGradient {
            centerX: wallpaperWindow.width * 0.33
            centerY: -wallpaperWindow.height * 0.33
            centerRadius: Math.min(wallpaperWindow.width, wallpaperWindow.height)
            focalX: centerX
            focalY: centerY
            focalRadius: 0

            GradientStop {
              position: 0
              color: Qt.rgba(Theme.primary.r, Theme.primary.g, Theme.primary.b, 0.25)
            }
            GradientStop {
              position: 1
              color: Qt.rgba(Theme.primary.r, Theme.primary.g, Theme.primary.b, 0)
            }
          }

          PathMove {
            x: 0
            y: 0
          }
          PathLine {
            x: wallpaperWindow.width + 1
            y: 0
          }
          PathLine {
            x: wallpaperWindow.width + 1
            y: wallpaperWindow.height + 1
          }
          PathLine {
            x: 0
            y: wallpaperWindow.height + 1
          }
          PathLine {
            x: 0
            y: 0
          }
        }
        ShapePath {
          fillGradient: RadialGradient {
            centerX: wallpaperWindow.width * 0.67
            centerY: wallpaperWindow.height * 1.33
            centerRadius: Math.min(wallpaperWindow.width, wallpaperWindow.height)
            focalX: centerX
            focalY: centerY
            focalRadius: 0

            GradientStop {
              position: 0
              color: Qt.rgba(Theme.primary.r, Theme.primary.g, Theme.primary.b, 0.25)
            }
            GradientStop {
              position: 1
              color: Qt.rgba(Theme.primary.r, Theme.primary.g, Theme.primary.b, 0)
            }
          }

          PathMove {
            x: 0
            y: 0
          }
          PathLine {
            x: wallpaperWindow.width + 1
            y: 0
          }
          PathLine {
            x: wallpaperWindow.width + 1
            y: wallpaperWindow.height + 1
          }
          PathLine {
            x: 0
            y: wallpaperWindow.height + 1
          }
          PathLine {
            x: 0
            y: 0
          }
        }
      }
      Item {
        anchors.centerIn: parent
        width: Math.min(wallpaperWindow.width, wallpaperWindow.height) / 3
        height: width

        VectorImage {
          anchors.fill: parent
          source: root.sourceUrl
          fillMode: VectorImage.PreserveAspectFit
          preferredRendererType: VectorImage.CurveRenderer
          asynchronous: true
          assumeTrustedSource: true
        }
      }
    }
  }
}
