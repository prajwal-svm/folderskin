#!/usr/bin/env bash
# What the app costs while it runs: memory that is really its own, CPU over a window, and the
# WebKit processes the window brings with it.
#
#   scripts/measure.sh              # the installed app
#   scripts/measure.sh 12345        # a pid you already have
#   scripts/measure.sh --watch 60   # a reading every 5s for a minute, as CSV
#
# Read `footprint`, not RSS: RSS counts shared framework pages the app didn't allocate, which is
# why a Tauri window "uses 400 MB" in ps and 80 MB in Activity Monitor. Footprint is what Activity
# Monitor calls Memory, and what the OS counts against the app.
set -uo pipefail

app_pid() {
  pgrep -f "FolderSkin.app/Contents/MacOS/folderskin" | head -1
}

# Physical footprint in MB: the app's own dirty and compressed pages.
footprint_mb() {
  local pid=$1
  local bytes
  bytes=$(vmmap -summary "$pid" 2>/dev/null | awk '/Physical footprint:/ {print $3; exit}')
  case "$bytes" in
    *G) echo "$bytes" | sed 's/G//' | awk '{printf "%.1f", $1 * 1024}' ;;
    *M) echo "$bytes" | sed 's/M//' | awk '{printf "%.1f", $1}' ;;
    *K) echo "$bytes" | sed 's/K//' | awk '{printf "%.1f", $1 / 1024}' ;;
    *) echo "0" ;;
  esac
}

# CPU percent over `window` seconds, from the change in cumulative CPU time — not ps's %cpu,
# which is an average over the whole life of the process and hides an idle spin.
cpu_over() {
  local pid=$1 window=${2:-5}
  local t0 t1
  t0=$(ps -o time= -p "$pid" | tr -d ' ')
  sleep "$window"
  t1=$(ps -o time= -p "$pid" | tr -d ' ')
  python3 - "$t0" "$t1" "$window" <<'PY'
import sys
def secs(t):
    parts = [float(p) for p in t.replace("-", ":").split(":")]
    out = 0.0
    for p in parts:
        out = out * 60 + p
    return out
a, b, w = secs(sys.argv[1]), secs(sys.argv[2]), float(sys.argv[3])
print(f"{(b - a) / w * 100:.1f}")
PY
}

# The WebKit XPC processes serving this app. They are children of launchd, not of the app, so the
# exact answer is `launchctl procinfo` — which needs root. Without it, match on start time: the
# window brings its helpers up within a second or two of the app, and nothing else on the machine
# usually starts a webview in that window. A helper WebKit restarted later is missed, and another
# WKWebView app opened at the same moment would be counted; both beat silently reporting a
# quarter of the memory, which is what asking for a `responsible pid` without root does.
HELPER_WINDOW=${HELPER_WINDOW:-10}

epoch() {
  date -j -f "%a %b %d %T %Y" "$1" +%s 2>/dev/null
}

webkit_pids() {
  local pid=$1 started begin
  # ps pads a single-digit day, so squeeze the spaces before parsing.
  started=$(ps -o lstart= -p "$pid" 2>/dev/null | tr -s ' ' | sed 's/^ *//; s/ *$//')
  [ -n "$started" ] || return 0
  begin=$(epoch "$started") || return 0
  ps -Ao pid,lstart,comm | grep -E "WebKit\.(WebContent|GPU|Networking)" |
    while read -r w day mon dd time year _; do
      local when
      when=$(epoch "$day $mon $dd $time $year") || continue
      if [ "$when" -ge "$begin" ] && [ $((when - begin)) -le "$HELPER_WINDOW" ]; then echo "$w"; fi
    done
}

# What the app reached earlier, and how much of what it holds is empty malloc pages the OS can
# take back under pressure — the two numbers that explain a footprint that moves on its own.
peak() {
  vmmap -summary "$1" 2>/dev/null | awk '
    function mb(v,   n, u) {
      n = v + 0; u = substr(v, length(v), 1)
      if (u == "G") return n * 1024
      if (u == "K") return n / 1024
      return n
    }
    /Physical footprint \(peak\):/ { peak = $4 }
    # Empty malloc regions: pages the allocator holds with nothing in them. The OS can take them
    # back under pressure, which is why a footprint falls on its own with the app untouched.
    /\(empty\)/ { empty += mb($5) }
    END { if (peak != "") printf "   (peak %s, %.0f MB held empty)", peak, empty }'
}

report() {
  local pid=$1
  local cpu total main
  if ! ps -p "$pid" >/dev/null 2>&1; then
    echo "no process $pid" >&2
    exit 1
  fi
  main=$(footprint_mb "$pid")
  cpu=$(cpu_over "$pid" "${WINDOW:-5}")
  total=$main
  echo "main          pid $pid   ${main} MB   ${cpu}% CPU$(peak "$pid")"
  for w in $(webkit_pids "$pid"); do
    local m c n
    m=$(footprint_mb "$w")
    c=$(cpu_over "$w" 1)
    n=$(ps -o comm= -p "$w" | sed 's/.*WebKit\.//; s/\.xpc.*//')
    total=$(python3 -c "print(f'{$total + $m:.1f}')")
    echo "  $n  pid $w   ${m} MB   ${c}% CPU"
  done
  echo "total         ${total} MB"
}

case "${1:-}" in
  --watch)
    pid=$(app_pid)
    end=$((SECONDS + ${2:-60}))
    echo "seconds,footprint_mb,cpu_percent"
    while [ $SECONDS -lt $end ]; do
      echo "$SECONDS,$(footprint_mb "$pid"),$(cpu_over "$pid" 5)"
    done
    ;;
  "")
    pid=$(app_pid)
    if [ -z "$pid" ]; then
      echo "FolderSkin isn't running. Open it, or pass a pid." >&2
      exit 1
    fi
    report "$pid"
    ;;
  *) report "$1" ;;
esac
