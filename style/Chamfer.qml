// Hand-calculated chamfer-cut geometry for bordered surfaces.
//
// Corner indexing: 0 = top-left, 1 = top-right, 2 = bottom-left,
// 3 = bottom-right.
//
// diagonal() returns the centerline endpoints of the diagonal band that
// replaces a square corner: the cut of `size` measured along each edge,
// inset by `half` (half the border width) so a stroke of the full border
// width stays inside the frame:
//
//   corner square: (0,0)-(size,size), cut from (size,0) to (0,size)
//   centerline:    (size,half) to (half,size)  [top-left case]
//
// Render with a ShapePath (strokeWidth = border width, round caps); plain
// rotated rectangles cannot antialias a thin diagonal cleanly.

pragma Singleton

import Quickshell

Singleton {
  function diagonal(corner, w, h, size, half) {
    switch (corner) {
    case 0:
      return {
        "ax": size,
        "ay": half,
        "bx": half,
        "by": size
      }
    case 1:
      return {
        "ax": w - size,
        "ay": half,
        "bx": w - half,
        "by": size
      }
    case 2:
      return {
        "ax": half,
        "ay": h - size,
        "bx": size,
        "by": h - half
      }
    default:
      return {
        "ax": w - size,
        "ay": h - half,
        "bx": w - half,
        "by": h - size
      }
    }
  }

  // Edge inset for a side endpoint next to a (possibly) chamfered corner.
  function edgeInset(chamfered, size) {
    return chamfered ? size : 0
  }
}
