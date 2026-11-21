#!/usr/bin/env bash
# End-to-end validation of DAG execution with automatic integration.
#
# The objective is decomposed, executed, merged into the goal's integration branch and
# promoted into the base branch. At the end the user's checkout contains the work and no
# `jcs/*` branches remain.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo not found" >&2; exit 1; }

MODEL=${JCS_E2E_MODEL:-opencode/hy4-flash-free}
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
JCS=("$BIN/jcs" --socket "$WORK/jcsd.sock")

step "fixture repository"
mkdir -p "$WORK/repo" "$WORK/data"
git -C "$WORK/repo" init -q -b main
git -C "$WORK/repo" config user.email merge@jcs.local
git -C "$WORK/repo" config user.name "Merge Fixture"
printf '# Fixture\n' > "$WORK/repo/README.md"
git -C "$WORK/repo" add -A
git -C "$WORK/repo" commit -qm "chore: initial commit"

export XDG_DATA_HOME="$WORK/data"
export JCS_DB="$WORK/jcs.db"
export JCS_SOCKET="$WORK/jcsd.sock"
export JCS_MAX_CONCURRENT=2
export RUST_LOG=warn

step "start daemon"
"$BIN/jcsd" > "$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 1 100); do
    if "${JCS[@]}" ping >/dev/null 2>&1; then break; fi
    sleep 0.1
done
"${JCS[@]}" ping >/dev/null 2>&1 || fail "daemon never became ready"

"${JCS[@]}" project add "$WORK/repo" --name merge > /dev/null
"${JCS[@]}" project settings merge --auto-merge true > /dev/null
"${JCS[@]}" project settings merge | grep -q "auto_merge=true" \
    || fail "auto_merge was not enabled"

step "ask for work that must be merged"
OBJECTIVE="Create ALPHA.txt containing the word alpha, then create BETA.txt containing the \
word beta. The second step depends on the first. Keep each file as its own step."

"${JCS[@]}" ask -p merge "$OBJECTIVE" --model "$MODEL" --detach --json > "$WORK/ask.json"
GOAL=$(python3 -c "import json,sys; print(json.load(open('$WORK/ask.json'))['goal_id'])")
[ -n "$GOAL" ] || fail "no goal id"

step "wait for the goal to settle"
STATUS=""
for _ in $(seq 1 180); do
    "${JCS[@]}" goal "$GOAL" --json > "$WORK/goal.json" || true
    STATUS=$(python3 -c "
import json
try:
    print(json.load(open('$WORK/goal.json'))['goal']['status'])
except Exception:
    print('')
")
    case "$STATUS" in
        succeeded|failed|blocked|cancelled) break ;;
    esac
    sleep 2
done
echo "status: $STATUS"
[ "$STATUS" = "succeeded" ] || {
    "${JCS[@]}" goal "$GOAL"
    fail "the goal did not succeed"
}

step "the base branch contains the work"
git -C "$WORK/repo" checkout -q main
for expected in ALPHA.txt BETA.txt; do
    git -C "$WORK/repo" ls-tree -r --name-only main | grep -qx "$expected" \
        || fail "$expected was not promoted into main"
done
echo "main tree: $(git -C "$WORK/repo" ls-tree -r --name-only main | tr '\n' ' ')"

step "the base branch advanced and the tree is clean"
[ "$(git -C "$WORK/repo" status --porcelain)" = "" ] || fail "the working tree is dirty"
git -C "$WORK/repo" log --oneline main | head -3

step "no integration or task branches remain"
REMAINING=$(git -C "$WORK/repo" branch --list 'jcs/*' | tr -d ' *+' || true)
[ -z "$REMAINING" ] || fail "branches left behind: $REMAINING"

step "the promotion is audited"
"${JCS[@]}" logs --limit 400 --json > "$WORK/logs.json"
grep -q '"result":"promoted"' "$WORK/logs.json" \
    || fail "no promoted merge event in the log"

printf '\nPASS: objective -> plan -> DAG -> integration -> main, branches cleaned up\n'
