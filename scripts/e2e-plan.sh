#!/usr/bin/env bash
# End-to-end validation of goal decomposition and DAG execution.
#
# The objective names three independent deliverables. The planner must produce more than
# one task, at least one dependency must exist (the objective states an order), every task
# must succeed, and every deliverable must exist on some task branch.
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
git -C "$WORK/repo" config user.email plan@jcs.local
git -C "$WORK/repo" config user.name "Plan Fixture"
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

"${JCS[@]}" project add "$WORK/repo" --name plan > /dev/null

step "ask for a decomposition"
OBJECTIVE="Create three small files. First create ALPHA.txt containing the word alpha. \
Then create BETA.txt containing the word beta, and GAMMA.txt containing the word gamma. \
The second step depends on the first. Each file is a separate, independently verifiable step."

"${JCS[@]}" ask -p plan "$OBJECTIVE" --model "$MODEL" --detach --json > "$WORK/ask.json"
GOAL=$(python3 -c "import json,sys; print(json.load(open('$WORK/ask.json'))['goal_id'])")
[ -n "$GOAL" ] || fail "no goal id"
echo "goal: $GOAL"

step "wait for the goal to settle"
STATUS=""
for _ in $(seq 1 150); do
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

step "the plan has more than one task and at least one dependency"
python3 - "$WORK/goal.json" <<'PY'
import json, sys
detail = json.load(open(sys.argv[1]))
tasks = detail["tasks"]
assert len(tasks) >= 2, f"expected a decomposition, got {len(tasks)} task(s)"
deps = [task for task in tasks if task.get("depends_on")]
assert deps, "expected at least one task with a dependency"
print(f"tasks: {len(tasks)}, with dependencies: {len(deps)}")
PY

step "every task succeeded"
python3 - "$WORK/goal.json" <<'PY'
import json, sys
detail = json.load(open(sys.argv[1]))
bad = [(task["id"], task["status"]) for task in detail["tasks"] if task["status"] != "succeeded"]
assert not bad, f"tasks not succeeded: {bad}"
print(f"all {len(detail['tasks'])} tasks succeeded")
PY

step "every deliverable exists on some task branch"
FOUND=""
for branch in $(git -C "$WORK/repo" branch --list 'jcs/*' | tr -d ' *+'); do
    FOUND="$FOUND $(git -C "$WORK/repo" ls-tree -r --name-only "$branch" | tr '\n' ' ')"
done
echo "files across branches:$FOUND"
for expected in ALPHA.txt BETA.txt GAMMA.txt; do
    echo "$FOUND" | grep -q "$expected" || fail "$expected was never written"
done

step "the plan was persisted"
"${JCS[@]}" logs --limit 300 --json > "$WORK/logs.json"
grep -q '"kind":"plan_created"' "$WORK/logs.json" || fail "no plan_created event in the log"

printf '\nPASS: objective -> plan -> DAG -> tasks -> files\n'
