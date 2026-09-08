pragma Singleton
import Quickshell

Singleton {
  function clamp(value, min, max) {
    return Math.max(min, Math.min(max, Number(value)))
  }
}
