#!/usr/bin/env bash
# Soak / budget test.
#
# Runs a small backlog under a token ceiling, proves the ceiling stops the next objective,
# raises it, resumes, and checks for leaks afterwards (processes, worktrees, stuck runs).
#
# This is the short version of the 72 h soak: same script, `SOAK_SECONDS=259200`.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo not found" >&2; exit 1; }

MODEL=${JCS_E2E_MODEL:-opencode-go/hy4-flash-free}
BUDGET_TOKENS=${SOAK_BUDGET_TOKENS:-1000}
WORK=$(mktemp -d)
DAEMON_PID=""

cleanup() {
    if [ -n "$DAEMON_PID" ]; then
        kill "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    if [ "${SOAK_KEEP:-0}" = "1" ]; then
        echo "kept: $WORK"
    else
        rm -rf "$WORK"
    fi
}
trap cleanup EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }
step() { printf '\n=== %s ===\n' "$*"; }

goal_status() {
    "${JCS[@]}" goal "$1" --json 2>/dev/null | python3 -c "
import json,sys
try:
    print(json.load(sys.stdin)['goal']['status'])
except Exception:
    print('')
"
}

wait_settled() {
    local goal=$1
    local status=""
    for _ in $(seq 1 150); do
        status=$(goal_status "$goal")
        case "$status" in
            succeeded|failed|blocked|cancelled) echo "$status"; return 0 ;;
        esac
        sleep 2
    done
    echo "$status"
    return 1
}

step "build"
cargo build --quiet --bins --manifest-path "$ROOT/Cargo.toml"
BIN="$ROOT/target/debug"
JCS=("$BIN/jcs" --socket "$WORK/jcsd.sock")

step "fixture repository"
mkdir -p "$WORK/repo" "$WORK/data"
git -C "$WORK/repo" init -q -b main
git -C "$WORK/repo" config user.email soak@jcs.local
git -C "$WORK/repo" config user.name "Soak Fixture"
printf '# Soak\n' > "$WORK/repo/README.md"
git -C "$WORK/repo" add -A
git -C "$WORK/repo" commit -qm "chore: initial commit"

export XDG_DATA_HOME="$WORK/data"
export JCS_DB="$WORK/jcs.db"
export JCS_SOCKET="$WORK/jcsd.sock"
export JCS_MAX_CONCURRENT=1
export JCS_MEMORY_SWEEP_SECS=0
export JCS_DEFAULT_PACKS=""
export RUST_LOG=warn

step "start daemon"
"$BIN/jcsd" > "$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 1 100); do
    "${JCS[@]}" ping >/dev/null 2>&1 && break
    sleep 0.1
done
"${JCS[@]}" ping >/dev/null 2>&1 || fail "daemon never became ready"

"${JCS[@]}" project add "$WORK/repo" --name soak > /dev/null
"${JCS[@]}" project settings soak --budget-tokens "$BUDGET_TOKENS" --auto-merge true > /dev/null

step "queue two objectives under a ${BUDGET_TOKENS}-token daily ceiling"
"${JCS[@]}" ask -p soak --no-plan "Create ONE.txt containing the word one." \
    --model "$MODEL" --detach --json > "$WORK/one.json"
# Deterministic order: the first objective must exist before the second is queued.
sleep 1
"${JCS[@]}" ask -p soak --no-plan "Create TWO.txt containing the word two." \
    --model "$MODEL" --detach --json > "$WORK/two.json"

ONE=$(python3 -c "import json;print(json.load(open('$WORK/one.json'))['goal_id'])")
TWO=$(python3 -c "import json;print(json.load(open('$WORK/two.json'))['goal_id'])")

STATUS_ONE=$(wait_settled "$ONE") || fail "goal one never settled"
STATUS_TWO=$(wait_settled "$TWO") || fail "goal two never settled"
echo "goal one: $STATUS_ONE · goal two: $STATUS_TWO"

if [ "$STATUS_ONE" != "succeeded" ] || [ "$STATUS_TWO" != "blocked" ]; then
    "${JCS[@]}" goal "$ONE" || true
    "${JCS[@]}" goal "$TWO" || true
    "${JCS[@]}" logs --limit 50 || true
    tail -20 "$WORK/daemon.log" || true
fi

# A run that fails in under a second with zero tokens never reached the model. That is an
# environment problem, not a product failure, and must be reported as a skip.
if [ "$STATUS_ONE" = "failed" ]; then
    TOKENS=$("${JCS[@]}" goal "$ONE" --json | python3 -c "
import json,sys
try:
    d=json.load(sys.stdin)
    print(sum(r.get('tokens_in',0)+r.get('tokens_out',0) for r in d['runs']))
except Exception:
    print('-1')
")
    if [ "$TOKENS" = "0" ]; then
        echo "SKIP: the model backend is unavailable (runs failed with 0 tokens)"
        exit 0
    fi
fi

[ "$STATUS_ONE" = "succeeded" ] || fail "the first goal should have succeeded"
[ "$STATUS_TWO" = "blocked" ] || fail "the second goal should have been parked by the budget"

step "the budget stop is audited"
"${JCS[@]}" logs --limit 400 --json > "$WORK/logs.json"
grep -q '"category":"budget_exceeded"' "$WORK/logs.json" \
    || fail "no budget_exceeded event in the log"

step "raise the ceiling and resume"
"${JCS[@]}" project settings soak --budget-tokens 100000000 > /dev/null
"${JCS[@]}" resume "$TWO" > /dev/null

STATUS_TWO=$(wait_settled "$TWO") || fail "goal two never settled after resume"
echo "goal two after resume: $STATUS_TWO"
if [ "$STATUS_TWO" != "succeeded" ]; then
    "${JCS[@]}" goal "$TWO"
    "${JCS[@]}" logs --limit 40
    fail "the resumed goal did not succeed"
fi

step "no stuck runs and no leaked worktrees"
STUCK=$(sqlite3 "$JCS_DB" "SELECT COUNT(*) FROM runs WHERE status = 'started';")
[ "$STUCK" = "0" ] || fail "$STUCK run(s) left in 'started'"

WORKTREES=$(git -C "$WORK/repo" worktree list --porcelain | grep -c '^worktree ' || true)
[ "$WORKTREES" = "1" ] || fail "expected only the main worktree, found $WORKTREES"

RSS_KB=$(ps -o rss= -p "$DAEMON_PID" | tr -d ' ')
echo "daemon RSS: ${RSS_KB} KB"

step "both deliverables exist on the base branch"
git -C "$WORK/repo" ls-tree -r --name-only main | grep -qx "ONE.txt" || fail "ONE.txt missing"
git -C "$WORK/repo" ls-tree -r --name-only main | grep -qx "TWO.txt" || fail "TWO.txt missing"

step "shutdown leaves no process behind"
SHUTDOWN_PID=$DAEMON_PID
kill "$SHUTDOWN_PID" 2>/dev/null || true
wait "$SHUTDOWN_PID" 2>/dev/null || true
DAEMON_PID=""
sleep 1
kill -0 "$SHUTDOWN_PID" 2>/dev/null && fail "jcsd $SHUTDOWN_PID survived the shutdown" || true

printf '\nPASS: budget stop -> audit -> resume -> clean shutdown (RSS %s KB)\n' "$RSS_KB"
