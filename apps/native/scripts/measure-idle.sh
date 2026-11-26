#!/usr/bin/env bash
# Locust at rest, measured the same way every time (speed-1 row 0).
#
# A SUBJECT copy of a given build runs over a subject data folder (never Colin's data, never Electron's folder). The
# default is a copy of the dogfood lane's subject-data made by `measure-idle.sh copy`: that lane's door process serves
# the original, and a second engine there would open its own door over the same connection file. The window activates
# itself when it opens (rune opens windows active), so `launch` and `front` are refused while someone is using the Mac
# (HID idle under SPEED_IDLE_MIN, default 300 s); `back` gives focus back to the app that had it before the launch.
# After a settle, the window and its engine are sampled for a window of seconds:
#   cpu %       CPU time used in the window / wall time (from `ps` TIME, exact to 10 ms; not top's instant figure)
#   idlew/s     top's idle wakeups (IDLEW, cumulative) counted across the window, per second
#   csw/s       top's context switches (CSW) across the window, per second: every wakeup of every thread
#   threads     top's #TH at the end
#   rss MB      resident memory at the end (`ps` RSS)
#   mem MB      top's MEM at the end: the footprint, compressed pages included
#   fds         open descriptors at the end (`lsof -d 0-65535`)
#   build load  rustc processes at the start (`pgrep -x rustc | wc -l`): the Mac is shared with lanes that compile
#
#   measure-idle.sh copy                               copy the dogfood subject-data (no checkpoints) to $DATA
#   measure-idle.sh launch <locust-native binary>     start a subject window (Home), print its pids
#   measure-idle.sh sample <label> [settle] [secs]    sample the running subject (default 20 s settle, 30 s window)
#   measure-idle.sh front                              bring the subject window forward (same idle gate)
#   measure-idle.sh back                               give focus back to the app that had it before the launch
#   measure-idle.sh quit                               end the subject window and its engine
#   measure-idle.sh run <binary> <label> [n]           launch, sample n times (default 3), quit: the Home scene
#   measure-idle.sh electron <label> [secs]            Electron Locust's own processes, read-only (nothing clicked)
#   measure-idle.sh pids <label> <secs> <name> <pid>.. any running processes, read-only (e.g. Colin's own copy)
#   measure-idle.sh rows                               print the table so far
#
# Rows go to $SPEED_OUT (default ~/work/scratch/evidence/speed-1/idle.tsv), one per sample, tab-separated.
set -euo pipefail

WORK="${SPEED_WORK:-$HOME/work/scratch/speed-1}"
SOURCE="$HOME/work/scratch/dogfood/subject-data"
DATA="${LOCUST_NATIVE_DATA:-$WORK/subject-data}"
OUT="${SPEED_OUT:-$HOME/work/scratch/evidence/speed-1/idle.tsv}"
PIDS="$WORK/subject.pids"
mkdir -p "$WORK" "$(dirname "$OUT")"

case "$DATA" in
  *"/Library/Application Support/@teammate"*) echo "refused: $DATA is Electron's folder" >&2; exit 2 ;;
esac

die() { echo "$*" >&2; exit 1; }

# Seconds since the last keyboard or mouse event on this Mac.
hid_idle() { ioreg -c IOHIDSystem | awk '/HIDIdleTime/ && !v {v=$NF} END {print int(v/1000000000)}'; }

now() { perl -MTime::HiRes=time -e 'printf "%.3f", time'; }

build_load() { pgrep -x rustc 2>/dev/null | wc -l | tr -d " "; }

# CPU seconds a process has used (ps TIME is [dd-]hh:mm:ss.cc or mm:ss.cc).
cpu_secs() {
  ps -o time= -p "$1" 2>/dev/null | awk '{
    t=$1; d=0; if (index(t,"-")) { split(t,a,"-"); d=a[1]; t=a[2] }
    n=split(t,p,":"); s=0; for (i=1;i<=n;i++) s=s*60+p[i]; printf "%.2f", s+d*86400 }'
}

# top's cumulative idle wakeups, thread count and memory footprint in MB (MEM, which counts compressed pages that
# ps RSS leaves out) and cumulative context switches for a pid: "IDLEW TH MEM CSW".
top_stats() {
  top -l 1 -pid "$1" -stats pid,idlew,th,mem,csw 2>/dev/null | awk -v p="$1" '$1==p {
    gsub(/[+\/].*/,"",$3); gsub(/\+/,"",$2); m=$4; gsub(/[+-]$/,"",m); u=substr(m,length(m)); v=substr(m,1,length(m)-1)+0
    if (u=="K") v=v/1024; else if (u=="G") v=v*1024; else if (u=="B") v=v/1048576; else if (u!="M") v=m/1048576
    c=$5; gsub(/[+-]/,"",c); printf "%s %s %.0f %s\n", $2, $3, v, c }'
}

rss_mb() { ps -o rss= -p "$1" 2>/dev/null | awk '{printf "%.0f", $1/1024}'; }

fds() { lsof -a -p "$1" -d 0-65535 2>/dev/null | awk 'NR>1' | wc -l | tr -d ' '; }

# The engine serving the subject data folder: the process holding its lock.
engine_pid() { lsof -t "$DATA/engine.lock" 2>/dev/null | awk 'NR==1' || true; }

away_or_die() {
  local idle; idle="$(hid_idle)"
  [ "$idle" -ge "${SPEED_IDLE_MIN:-300}" ] || die "refused: the Mac was used ${idle}s ago; a subject window would take the screen"
}

frontmost() { osascript -e 'tell application "System Events" to get bundle identifier of first process whose frontmost is true' 2>/dev/null || true; }

copy_data() {
  [ -d "$SOURCE" ] || die "no $SOURCE"
  [ -f "$PIDS" ] && die "a subject is running; quit it first"
  mkdir -p "$DATA"
  rsync -a --delete --exclude checkpoints --exclude 'engine.*' --exclude 'opencode-guard-*' --exclude 'locust-mcp-connection.json' "$SOURCE/" "$DATA/"
  echo "copied $SOURCE -> $DATA ($(du -sh "$DATA" | cut -f1))"
}

launch() {
  local bin="$1"
  [ -d "$DATA" ] || die "no $DATA; run: $0 copy"
  away_or_die
  [ -x "$bin" ] || die "not an executable: $bin"
  [ -f "$PIDS" ] && die "a subject is already running ($(cat "$PIDS")); run: $0 quit"
  [ -n "$(engine_pid)" ] && die "an engine already holds $DATA/engine.lock (pid $(engine_pid)); not mine to stop"
  local sum; sum="$(shasum -a 256 "$bin" | cut -c1-12)"
  frontmost > "$WORK/frontmost"
  # Started from this shell, not through LaunchServices: an app LaunchServices starts asks macOS for access to the
  # external drive the data folder is on, and waits in open() behind that prompt until someone answers it.
  (cd / && export LOCUST_NATIVE_ENGINE=1 LOCUST_NATIVE_DATA="$DATA" && exec nohup "$bin" > "$WORK/subject.out" 2> "$WORK/subject.err" < /dev/null) &
  local win=$! eng="" i
  for i in $(seq 1 100); do
    eng="$(engine_pid || true)"
    [ -n "$eng" ] && break
    kill -0 "$win" 2>/dev/null || die "the subject window ended at once; see $WORK/subject.err"
    sleep 0.1
  done
  [ -n "$eng" ] || echo "warning: no engine holds $DATA/engine.lock yet" >&2
  echo "$win ${eng:-0} $sum" > "$PIDS"
  echo "subject window $win, engine ${eng:-none}, build $sum"
}

read_pids() { [ -f "$PIDS" ] || die "no subject running; run: $0 launch <binary>"; read -r WIN ENG SUM < "$PIDS"; }

# Sample a set of pids for $secs; print one row per pid plus a total.
measure() {
  local label="$1" secs="$2"; shift 2
  local names=() pids=() c0=() w0=() k0=() load t0 t1
  while [ $# -gt 0 ]; do names+=("$1"); pids+=("$2"); shift 2; done
  load="$(build_load)"
  t0="$(now)"
  for i in "${!pids[@]}"; do
    c0[$i]="$(cpu_secs "${pids[$i]}")"; read -r w _ _ k < <(top_stats "${pids[$i]}"); w0[$i]="${w:-0}"; k0[$i]="${k:-0}"
  done
  sleep "$secs"
  t1="$(now)"
  local wall; wall="$(awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.2f", b-a}')"
  local tc=0 tw=0 tr=0 tm=0 tk=0
  for i in "${!pids[@]}"; do
    local p="${pids[$i]}" c1 w1 th mem k1 rss fd cpu wps cps
    c1="$(cpu_secs "$p")"; read -r w1 th mem k1 < <(top_stats "$p"); rss="$(rss_mb "$p")"; fd="$(fds "$p")"
    cps="$(awk -v a="${k0[$i]}" -v b="${k1:-0}" -v w="$wall" 'BEGIN{printf "%.0f", (b-a)/w}')"
    cpu="$(awk -v a="${c0[$i]}" -v b="$c1" -v w="$wall" 'BEGIN{printf "%.2f", (b-a)*100/w}')"
    wps="$(awk -v a="${w0[$i]}" -v b="${w1:-0}" -v w="$wall" 'BEGIN{printf "%.1f", (b-a)/w}')"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$(date '+%m-%d %H:%M:%S')" "$label" "${names[$i]}" "$cpu" "$wps" "$cps" "${th:-?}" "$rss" "${mem:-?}" "$fd" "$load" "${SUM:-}" | tee -a "$OUT"
    tc="$(awk -v a="$tc" -v b="$cpu" 'BEGIN{print a+b}')"; tw="$(awk -v a="$tw" -v b="$wps" 'BEGIN{print a+b}')"; tk=$((tk + cps)); tr=$((tr + ${rss:-0})); tm=$((tm + ${mem:-0}))
  done
  printf '%s\t%s\t%s\t%.2f\t%.1f\t%s\t-\t%s\t%s\t-\t%s\t%s\n' "$(date '+%m-%d %H:%M:%S')" "$label" "TOTAL" "$tc" "$tw" "$tk" "$tr" "$tm" "$load" "${SUM:-}" | tee -a "$OUT"
}

header() {
  [ -s "$OUT" ] || printf 'when\tlabel\tprocess\tcpu%%\tidlew/s\tcsw/s\tthreads\trss MB\tmem MB\tfds\trustc\tbuild\n' > "$OUT"
}

sample() {
  local label="$1" settle="${2:-20}" secs="${3:-30}"
  read_pids
  header
  sleep "$settle"
  kill -0 "$WIN" 2>/dev/null || die "the subject window $WIN is gone"
  [ "$ENG" != 0 ] || { ENG="$(engine_pid || echo 0)"; }
  if [ "$ENG" != 0 ] && kill -0 "$ENG" 2>/dev/null; then
    measure "$label" "$secs" window "$WIN" engine "$ENG"
  else
    measure "$label" "$secs" window "$WIN"
  fi
}

front() {
  read_pids
  away_or_die
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $WIN) to true" >/dev/null
}

back() {
  read_pids
  local was; was="$(cat "$WORK/frontmost" 2>/dev/null || true)"
  [ -n "$was" ] || was=com.apple.finder
  osascript -e "tell application id \"$was\" to activate" >/dev/null
}

quit_subject() {
  read_pids
  kill -TERM "$WIN" 2>/dev/null || true
  local i
  for i in $(seq 1 100); do kill -0 "$WIN" 2>/dev/null || break; sleep 0.1; done
  kill -0 "$WIN" 2>/dev/null && kill "$WIN" 2>/dev/null || true
  # Quitting the window shuts its engine down (decision 3); one still here after 10 s is ended.
  for i in $(seq 1 100); do [ "$ENG" = 0 ] || kill -0 "$ENG" 2>/dev/null || break; sleep 0.1; done
  [ "$ENG" != 0 ] && kill -0 "$ENG" 2>/dev/null && kill "$ENG" 2>/dev/null || true
  rm -f "$PIDS"
  echo "subject window $WIN and engine $ENG ended"
}

electron() {
  local label="$1" secs="${2:-30}" args=() p
  header
  for p in $(pgrep -f "^/Applications/Locust.app/Contents/" || true); do
    case "$(ps -o command= -p "$p")" in
      *crashpad*) ;;
      *"--type=gpu-process"*) args+=(gpu "$p") ;;
      *"--type=renderer"*) args+=(renderer "$p") ;;
      *"--type=utility"*) args+=(utility "$p") ;;
      *) args+=(main "$p") ;;
    esac
  done
  [ ${#args[@]} -gt 0 ] || die "Electron Locust is not running"
  SUM=electron measure "$label" "$secs" "${args[@]}"
}

case "${1:-}" in
  copy) copy_data ;;
  launch) launch "${2:?binary}" ;;
  sample) sample "${2:?label}" "${3:-20}" "${4:-30}" ;;
  front) front ;;
  back) back ;;
  quit) quit_subject ;;
  run)
    launch "${2:?binary}"
    trap '"$0" quit || true' EXIT
    for n in $(seq 1 "${4:-3}"); do sample "${3:?label}" "$([ "$n" = 1 ] && echo 20 || echo 2)" 30; done
    ;;
  electron) electron "${2:?label}" "${3:-30}" ;;
  pids) header; label="${2:?label}"; secs="${3:?secs}"; shift 3; SUM=other measure "$label" "$secs" "$@" ;;
  rows) column -t -s $'\t' "$OUT" ;;
  *) sed -n '2,26p' "$0"; exit 2 ;;
esac
