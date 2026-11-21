#!/usr/bin/env bash
# Durability test: kill the daemon with SIGKILL mid-run, restart, verify honest recovery.
#
# The properties proven here are the ones that make autonomous execution safe to trust:
#   * a killed run is reported as interrupted, with a reason;
#   * the task is parked as blocked, not silently failed or silently retried;
#   * no partial commit exists on the task branch;
#   * the stale socket is detected and replaced;
#   * the daemon works normally afterwards.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# rustup may have been installed without touching the shell profile.
command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo not found; install Rust first" >&2; exit 1; }
MODEL=${JCS_E2E_MODEL:-opencode/hy4-flash-free}
WORK=$(mktemp -d)
DAEMON_PID=""
RUNNING=0

cleanup() {
    if [ "$RUNNING" = "1" ] && [ -n "$DAEMON_PID" ]; then
        kill "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }
step() { printf '\n=== %s ===\n' "$*"; }
start_daemon() {
    "$BIN/jcsd" >> "$WORK/daemon.log" 2>&1 &
    DAEMON_PID=$!
    RUNNING=1
    # A socket file is not readiness: after SIGKILL the old one is still there.
    for _ in $(seq 1 100); do
        if "${JCS[@]}" ping >/dev/null 2>&1; then return 0; fi
        sleep 0.1
    done
    fail "daemon never became ready; log: $(cat "$WORK/daemon.log")"
}
stop_daemon() {
    [ -n "$DAEMON_PID" ] || return 0
    kill "$DAEMON_PID" 2>/dev/null || true
    wait "$DAEMON_PID" 2>/dev/null || true
    DAEMON_PID=""
    RUNNING=0
}

step "build"
cargo build --quiet --bins --manifest-path "$ROOT/Cargo.toml"
BIN="$ROOT/target/debug"
JCS=("$BIN/jcs" --socket "$WORK/jcsd.sock")

step "fixture repository"
mkdir -p "$WORK/repo" "$WORK/data"
git -C "$WORK/repo" init -q -b main
git -C "$WORK/repo" config user.email chaos@jcs.local
git -C "$WORK/repo" config user.name "Chaos Fixture"
printf '# Chaos\n' > "$WORK/repo/README.md"
git -C "$WORK/repo" add -A
git -C "$WORK/repo" commit -qm "chore: initial commit"

export XDG_DATA_HOME="$WORK/data"
export JCS_DB="$WORK/jcs.db"
export JCS_SOCKET="$WORK/jcsd.sock"
export JCS_MAX_CONCURRENT=1
export RUST_LOG=warn

step "start, launch a run, kill -9 mid-flight"
start_daemon
"${JCS[@]}" project add "$WORK/repo" --name chaos > /dev/null
"${JCS[@]}" ask -p chaos "Create NOTES.md with the single line: written during the chaos test." \
    --model "$MODEL" --detach > /dev/null
sleep 4
kill -9 "$DAEMON_PID"
wait "$DAEMON_PID" 2>/dev/null || true
RUNNING=0
[ -S "$JCS_SOCKET" ] || fail "expected a stale socket after SIGKILL"

step "restart"
start_daemon

step "recovery is honest"
"${JCS[@]}" runs | grep -q "interrupted" || fail "the killed run is not reported as interrupted"
"${JCS[@]}" status | grep -q "blocked" || fail "the task was not parked as blocked"
"${JCS[@]}" logs --limit 200 | grep -q "run interrupted" || fail "no interrupted event in the log"

step "no partial work was committed"
for branch in $(git -C "$WORK/repo" branch --list 'jcs/*' | tr -d ' *+'); do
    PARTIAL=$(git -C "$WORK/repo" log --oneline "main..$branch")
    [ -z "$PARTIAL" ] || {
        echo "$PARTIAL"
        fail "partial commits found on $branch"
    }
done

step "the daemon still works after recovery"
"${JCS[@]}" ask -p chaos "Create AFTER.md containing the single line: recovered fine." \
    --model "$MODEL" > "$WORK/after.txt" 2>&1 || {
    cat "$WORK/after.txt"
    fail "a run after recovery failed"
}
grep -q "run succeeded" "$WORK/after.txt" || fail "the post-recovery run did not succeed"

AFTER_BRANCH=$(git -C "$WORK/repo" worktree list --porcelain | awk '/^branch refs\/heads\/jcs\/create-after/{sub("refs/heads/","",$2); print $2; exit}')
[ -n "$AFTER_BRANCH" ] || fail "the post-recovery run produced no branch"
git -C "$WORK/repo" show "$AFTER_BRANCH:AFTER.md" | grep -q "recovered fine" \
    || fail "the post-recovery commit is missing its content"

stop_daemon

printf '\nPASS: kill -9 -> interrupted + blocked, no partial commits, recovery works\n'
