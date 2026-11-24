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
  // Lock header, measured from the shipped font files (advance widths in em):
  // widest date "08 May 2026" (with 2026/2027 and 1..31 Jan..Dec) = 5.28em.
  // At 48px that is the target width every header line is fitted to.
  // Widest weekday "WEDNESDAY" = 5.069em -> 50px matches the target.
  // Widest clock "08:48" = 2.365em -> 107.16px matches the target; the
  // clock itself gets no letter spacing, day/date are fitted dynamically.
  readonly property real lockDate: 48
  readonly property real lockDay: 50
  readonly property real lockTime: 107.16
  readonly property real lockHeaderWidth: 5.28 * lockDate

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
