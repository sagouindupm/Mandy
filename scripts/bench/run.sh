#!/usr/bin/env bash
# scripts/bench/run.sh <label> <victory.exe> [VAR=value ...]
# One measured run (5 minutes) of a build from the SAME save every time, in Git Bash on Windows.
# Prints: world-frame median and p90 (frames with 300+ draws), RAM peak, VRAM, GPU losses; saves
# 3 screenshots and the log in $BENCH_OUT. Compare two builds or settings by alternating runs
# (A1 B1 A2 B2): one run is +-100 ms of noise, two rounds at least.
#
# Set once (your paths):
#   BENCH_GAME   = path of your game_scarlett_master.exe (Windows path)
#   BENCH_CONFIG = your victory.toml (Windows path)
#   BENCH_SAVES  = the folder VIctory keeps the title's saves in (data dir \ <title id> \ saves)
#   BENCH_SAVE   = a copy of that folder taken at the place you measure (restored before each run:
#                  the title autosaves, so without it every run starts somewhere else)
#   BENCH_OUT    = where logs, screenshots and results go
# Optional: CONSOLE (series-s), KEYARGS (keys.ps1 -Vk/-Scan for a build that maps A elsewhere).
label=$1; exe=$2; shift 2
: "${BENCH_GAME:?set BENCH_GAME}" "${BENCH_CONFIG:?set BENCH_CONFIG}" "${BENCH_OUT:?set BENCH_OUT}"
here=$(cd "$(dirname "$0")" && pwd -W 2>/dev/null || pwd)
mkdir -p "$BENCH_OUT"; LOG=$BENCH_OUT/run_$label.log
if tasklist | grep -qi victory.exe; then echo "$label: victory already running"; exit 1; fi
if [ -n "${BENCH_SAVE:-}" ] && [ -n "${BENCH_SAVES:-}" ]; then rm -rf "$BENCH_SAVES"; cp -r "$BENCH_SAVE" "$BENCH_SAVES"; fi
export VICTORY_CONFIG="$BENCH_CONFIG"
env "$@" "$exe" run "$BENCH_GAME" --execute --console ${CONSOLE:-series-s} > "$LOG" 2>&1 &
sleep 15
# Presses A (Enter) in the game window through the menus; logged per run.
powershell -NoProfile -File "$here\keys.ps1" ${KEYARGS:-} -At '5,8,12,17,22,28,38,48,60,75,90,105,120' > "$BENCH_OUT/keys_$label.txt" 2>&1 &
peak=0
for i in $(seq 1 29); do
    sleep 10
    tasklist | grep -qi victory.exe || { echo "$label: process ended at $((i*10+15)) s"; break; }
    ws=$(powershell -NoProfile -Command "[int]((Get-Process victory -EA 0 | measure WorkingSet64 -Sum).Sum/1MB)")
    [ "${ws:-0}" -gt "$peak" ] && peak=$ws
    case $i in 15|22|29) powershell -NoProfile -File "$here\shotwin.ps1" -Out "$(cygpath -w "$BENCH_OUT")\run_${label}_$i.png" > /dev/null 2>&1;; esac
done
vram=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader 2>/dev/null | tr -d '\n')
taskkill //IM victory.exe //T //F > /dev/null 2>&1; for w in $(seq 1 30); do sleep 2; tasklist | grep -qi victory.exe || break; done; sleep 2
lost=$(sed 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -c 'device removed')
sed 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -oE '[0-9.]+ ms/frame.* ([0-9.]+) draws/frame' | awk -v l="$label" -v p="$peak" -v vr="$vram" -v lost="$lost" '
    { ms = $1 + 0; d = $(NF-1) + 0; if (d >= 300) { n++; v[n] = ms } }
    END { if (!n) { printf "%s: no world frames | RAM %d MB | VRAM %s | GPU lost %d\n", l, p, vr, lost; exit }
          for (i = 1; i <= n; i++) for (j = i + 1; j <= n; j++) if (v[j] < v[i]) { t = v[i]; v[i] = v[j]; v[j] = t }
          m = v[int((n+1)/2)]; k = int(n*0.9); if (k < 1) k = 1; p90 = v[k]
          printf "%s: %d world lines, median %.0f ms (%.1f fps), p90 %.0f ms (%.1f fps), best %.0f, worst %.0f | RAM peak %d MB | VRAM %s | GPU lost %d\n", l, n, m, 1000/m, p90, 1000/p90, v[1], v[n], p, vr, lost }'
