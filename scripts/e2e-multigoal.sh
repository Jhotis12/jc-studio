#!/usr/bin/env bash
# Two objectives, one project, four slots: both must make progress at the same time.
#
# This is the regression test for the "seven tasks, one by one" report. It also proves that
# concurrent agents no longer trip SQLite's deferred-transaction lock.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo not found" >&2; exit 1; }

MODEL=${JCS_E2E_MODEL:-opencode-go/hy4-flash-free}
WORK=$(mktemp -d)
DAEMON_PID=""

cleanup() {
    if [ -n "$DAEMON_PID" ]; then
        kill "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }
step() { printf '\n=== %s ===\n' "$*"; }

step "build"
cargo build --quiet --bins --manifest-path "$ROOT/Cargo.toml"
BIN="$ROOT/target/debug"

step "fixture repository"
mkdir -p "$WORK/repo" "$WORK/data"
git -C "$WORK/repo" init -q -b main
git -C "$WORK/repo" config user.email multi@jcs.local
git -C "$WORK/repo" config user.name "Multi Goal Fixture"
printf '# Fixture\n' > "$WORK/repo/README.md"
git -C "$WORK/repo" add -A
git -C "$WORK/repo" commit -qm "chore: initial commit"

export XDG_DATA_HOME="$WORK/data"
export JCS_DB="$WORK/jcs.db"
export JCS_SOCKET="$WORK/jcsd.sock"
export JCS_MAX_CONCURRENT=4
export JCS_MODEL="$MODEL"
export RUST_LOG=warn

step "start daemon"
"$BIN/jcsd" > "$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 1 100); do
    curl -s --unix-socket "$JCS_SOCKET" http://localhost/health >/dev/null 2>&1 && break
    sleep 0.1
done
curl -s --unix-socket "$JCS_SOCKET" http://localhost/health >/dev/null \
    || fail "daemon never became ready"

"$BIN/jcs" --socket "$JCS_SOCKET" project add "$WORK/repo" --name multi >/dev/null

step "queue two independent objectives"
"$BIN/jcs" --socket "$JCS_SOCKET" ask -p multi --detach \
    "Crea cuatro archivos independientes en alpha/: a1.txt, a2.txt, a3.txt y a4.txt, cada uno con una frase corta." \
    >/dev/null
"$BIN/jcs" --socket "$JCS_SOCKET" ask -p multi --detach \
    "Crea cuatro archivos independientes en beta/: b1.txt, b2.txt, b3.txt y b4.txt, cada uno con una frase corta." \
    >/dev/null

step "both objectives must be working at the same time"
BOTH=0
for _ in $(seq 1 180); do
    curl -s --unix-socket "$JCS_SOCKET" http://localhost/status > "$WORK/status.json"
    RESULT=$(python3 - "$WORK/status.json" <<'PY'
import json, sys
status = json.load(open(sys.argv[1]))
goals = {g["id"]: g for g in status["goals"]}
running = {}
for task in status["tasks"]:
    if task["status"] in ("running", "verifying"):
        running[task["goal_id"]] = running.get(task["goal_id"], 0) + 1
if len(running) >= 2 and all(count >= 1 for count in running.values()):
    print("both")
elif goals and all(g["status"] in ("succeeded", "failed", "cancelled", "blocked") for g in goals.values()):
    print("done")
else:
    print("wait")
PY
)
    case "$RESULT" in
        both) BOTH=1; break ;;
        done) break ;;
    esac
    sleep 1
done

[ "$BOTH" = "1" ] || fail "the two objectives never ran concurrently"

step "wait for both objectives to settle"
STATUS=""
for _ in $(seq 1 240); do
    curl -s --unix-socket "$JCS_SOCKET" http://localhost/status > "$WORK/status.json"
    STATUS=$(python3 - "$WORK/status.json" <<'PY'
import json, sys
status = json.load(open(sys.argv[1]))
goals = status["goals"]
if goals and all(g["status"] in ("succeeded", "failed", "cancelled", "blocked") for g in goals):
    print(",".join(sorted(g["status"] for g in goals)))
else:
    print("running")
PY
)
    [ "$STATUS" = "running" ] || break
    sleep 1
done
echo "goals: $STATUS"
[ "$STATUS" = "succeeded,succeeded" ] || fail "not every objective succeeded: $STATUS"

step "the database was never locked"
if grep -q "database is locked" "$WORK/daemon.log"; then
    grep "database is locked" "$WORK/daemon.log" | tail -3
    fail "a concurrent agent hit SQLite's lock"
fi

step "no task or run is left in flight"
LEFTOVER=$(python3 - "$WORK/status.json" <<'PY'
import json, sys
status = json.load(open(sys.argv[1]))
print(sum(1 for task in status["tasks"] if task["status"] in ("running", "verifying")))
PY
)
[ "$LEFTOVER" = "0" ] || fail "$LEFTOVER tasks are still running"

echo
echo "PASS: two objectives ran concurrently and both succeeded"
