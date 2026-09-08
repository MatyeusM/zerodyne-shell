pragma Singleton
import Quickshell

Singleton {
  property real remPx: 16

  function isValid(value) {
    if (typeof value !== "number" || !Number.isFinite(value))
      return false
    if (value >= 4)
      return Number.isInteger(value)
    if (value >= 1)
      return Number.isInteger(value * 4)
    if (value >= 0)
      return Number.isInteger(value * 16)
    return false
  }
  function rem(multiplier) {
    if (!isValid(multiplier))
      throw new Error("Base.rem: invalid multiplier " + multiplier)
    return multiplier * remPx
  }
}
