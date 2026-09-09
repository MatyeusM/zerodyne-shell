import QtQuick
import Quickshell
import Quickshell.Io

// App model + fuzzy filter + launch for the rofi replacement. The native
// DesktopEntries singleton owns the app list (live, all data dirs);
// entry.execute() launches with field codes, terminal handling and
// startup notification, so no scripts or parsers are needed here.
// results holds live entry refs in a plain array (never in a ListModel:
// QObjects in model roles dangle when the source updates them).
// NoDisplay entries never reach the results (filtered upstream).
//
// Rofi-style frequency presort: every launch bumps a counter persisted
// to the cache dir (debounced). An empty query sorts by count first; a
// typed query still ranks match quality first with the count breaking
// ties. Cache, not state: counts are regenerable statistics, and a
// `rm -rf ~/.cache` should reset them, not wipe user configuration.
Item {
  id: root

  property string query: ""
  property var results: []
  property int selected: 0
  readonly property int maxResults: 10
  readonly property string cacheDir: Quickshell.env("HOME") + "/.cache/zerodyne/"
  readonly property string freqPath: cacheDir + "launcher.json"
  property var frequencies: ({})
  property bool freqLoaded: false

  function combinedText(entry) {
    var parts = []
    parts.push(entry.name || "")
    parts.push(entry.genericName || "")
    var keywords = entry.keywords || []
    for (var i = 0; i < keywords.length; i++) {
      parts.push(keywords[i])
    }
    parts.push(entry.comment || "")
    return parts.join(" ")
  }
  // Subsequence fuzzy score: -1 is no match, otherwise higher is better.
  // Name-prefix beats substring beats a compact subsequence; the name
  // leads the combined text so it naturally outranks other fields.
  function matchScore(text, q) {
    var t = String(text || "").toLowerCase()
    if (q.length === 0) {
      return 1
    }
    var head = t.indexOf(q)
    if (head === 0) {
      return 1000 - t.length
    }
    if (head > 0) {
      return 500 - head
    }
    var at = 0
    var first = -1
    var prev = -1
    var gaps = 0
    for (var i = 0; i < q.length; i++) {
      var hit = t.indexOf(q.charAt(i), at)
      if (hit < 0) {
        return -1
      }
      if (first < 0) {
        first = hit
      }
      if (prev >= 0) {
        gaps += hit - prev - 1
      }
      prev = hit
      at = hit + 1
    }
    var score = 200 - first - gaps
    return score > 0 ? score : 0
  }
  function freqOf(entry) {
    var n = Number(root.frequencies[entry.id] || 0)
    return isFinite(n) && n > 0 ? n : 0
  }
  function compareNames(a, b) {
    var an = String(a.name || "").toLowerCase()
    var bn = String(b.name || "").toLowerCase()
    if (an < bn) {
      return -1
    }
    if (an > bn) {
      return 1
    }
    return 0
  }
  function refresh() {
    var q = root.query.toLowerCase()
    var all = DesktopEntries.applications.values
    var hits = []
    for (var i = 0; i < all.length; i++) {
      var entry = all[i]
      if (!entry || entry.noDisplay) {
        continue
      }
      if (root.matchScore(root.combinedText(entry), q) < 0) {
        continue
      }
      hits.push(entry)
    }
    if (q.length === 0) {
      hits.sort(function (a, b) {
        var fa = root.freqOf(a)
        var fb = root.freqOf(b)
        if (fa !== fb) {
          return fb - fa
        }
        return root.compareNames(a, b)
      })
    } else {
      hits.sort(function (a, b) {
        var sa = root.matchScore(root.combinedText(a), q)
        var sb = root.matchScore(root.combinedText(b), q)
        if (sa !== sb) {
          return sb - sa
        }
        var fa = root.freqOf(a)
        var fb = root.freqOf(b)
        if (fa !== fb) {
          return fb - fa
        }
        return root.compareNames(a, b)
      })
    }
    root.results = hits.slice(0, root.maxResults)
    if (root.selected >= root.results.length) {
      root.selected = Math.max(0, root.results.length - 1)
    }
  }
  function move(delta) {
    if (root.results.length === 0) {
      return
    }
    var next = root.selected + delta
    if (next < 0) {
      next = 0
    }
    if (next >= root.results.length) {
      next = root.results.length - 1
    }
    root.selected = next
  }
  function recordLaunch(entry) {
    if (!entry || !entry.id) {
      return
    }
    var counts = root.frequencies
    counts[entry.id] = root.freqOf(entry) + 1
    root.frequencies = counts
    root.scheduleFreqSave()
  }
  function launchAt(i) {
    if (i < 0 || i >= root.results.length) {
      return
    }
    var entry = root.results[i]
    root.recordLaunch(entry)
    try {
      entry.execute()
    } catch (e) {}
  }
  function launchSelected() {
    root.launchAt(root.selected)
  }
  function scheduleFreqSave() {
    if (!root.freqLoaded) {
      return
    }
    freqSaveTimer.restart()
  }
  function loadFreqs(raw) {
    if (root.freqLoaded) {
      return
    }
    var counts = {}
    try {
      var parsed = JSON.parse(String(raw || ""))
      if (parsed && typeof parsed === "object") {
        for (var key in parsed) {
          var n = Number(parsed[key])
          if (key && isFinite(n) && n > 0) {
            counts[key] = Math.floor(n)
          }
        }
      }
    } catch (e) {}
    root.frequencies = counts
    root.freqLoaded = true
    root.refresh()
  }

  onQueryChanged: {
    root.selected = 0
    root.refresh()
  }
  Component.onCompleted: {
    ensureCacheDir.running = true
    root.refresh()
    // FileView reads an empty string when the file is missing; the mkdir
    // gets a tick first so a first-run load is a clean miss, not an error.
    Qt.callLater(function () {
      freqFile.reload()
    })
  }

  Connections {
    target: DesktopEntries.applications

    onValuesChanged: {
      root.refresh()
    }
  }

  // Counts only change on launch, so a short debounce coalesces bursts
  // without losing anything.
  Timer {
    id: freqSaveTimer

    interval: 500
    repeat: false

    onTriggered: freqFile.setText(JSON.stringify(root.frequencies) + "\n")
  }
  Process {
    id: ensureCacheDir

    command: ["mkdir", "-p", root.cacheDir]
    running: false
  }
  FileView {
    id: freqFile

    path: root.freqPath
    watchChanges: false
    atomicWrites: true
    printErrors: false

    onLoaded: root.loadFreqs(text())
    onLoadFailed: root.loadFreqs("")
  }
}
