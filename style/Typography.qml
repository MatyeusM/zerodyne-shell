pragma Singleton
import QtQuick
import Quickshell

Singleton {
  property var rajdhaniWeights: ["Light", "Regular", "Medium", "SemiBold", "Bold"]
  property var monoWeights: ["Thin", "ExtraLight", "Light", "Regular", "Medium", "SemiBold", "Bold",
    "ExtraBold"]

  property string displayName: "Rajdhani"
  property string monoName: "JetBrainsMono Nerd Font Mono"
  readonly property real xs: Base.rem(0.75)
  readonly property real sm: Base.rem(0.875)
  readonly property real md: Base.rem(1)
  readonly property real lg: Base.rem(1.25)
  readonly property real xl: Base.rem(1.5)
  readonly property real xxl: Base.rem(1.75)

  function font(options, defaults) {
    options = Object.assign(defaults, options || {})
    return {
      "family": options.name,
      "pixelSize": options.size,
      "weight": options.weight
    }
  }
  function display(options) {
    return font(options, {
                  "name": displayName,
                  "size": md,
                  "weight": Font.Bold
                })
  }
  function mono(options) {
    return font(options, {
                  "name": monoName,
                  "size": md,
                  "weight": Font.Bold
                })
  }

  Instantiator {
    model: rajdhaniWeights

    FontLoader {
      source: Quickshell.shellPath(`assets/rajdhani/Rajdhani-${modelData}.ttf`)
    }
  }
  Instantiator {
    model: monoWeights

    FontLoader {
      source: Quickshell.shellPath(`assets/jetbrains-mono/JetBrainsMonoNerdFontMono-${modelData
                                   }.ttf`)
    }
  }
}
