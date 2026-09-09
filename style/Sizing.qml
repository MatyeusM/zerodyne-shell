pragma Singleton
import Quickshell

Singleton {
  readonly property real xxxs: Base.rem(16)
  readonly property real xxs: Base.rem(18)
  readonly property real xs: Base.rem(20)
  readonly property real sm: Base.rem(24)
  readonly property real md: Base.rem(28)
  readonly property real lg: Base.rem(32)
  readonly property real xl: Base.rem(36)
  readonly property real xxl: Base.rem(42)
  // Fixed bar indicator width shared by the clock neighbors (audio,
  // connectivity) so both arms measure the same and the clock stays
  // visually centered. Sized to fit the audio worst case (pct + 3 icons
  // + gaps + insets); the narrower indicator centers its content.
  readonly property real barIndicatorWidth: Base.rem(10)
}
