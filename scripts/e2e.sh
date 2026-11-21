#!/usr/bin/env bash
# End-to-end validation of the walking skeleton.
#
# Proves, against a real OpenCode backend and a throwaway git repository, that a single
# objective becomes an isolated worktree, an agent session, passing gates and a commit.
#
# Cost: uses a free model by default. Override with JCS_E2E_MODEL.
# Requirements: `opencode service status` must report a running service.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# rustup may have been installed without touching the shell profile.
command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo not found; install Rust first" >&2; exit 1; }
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
wait_for_daemon() {
    # A socket file is not readiness: after a crash the old one is still there.
    for _ in $(seq 1 100); do
        if "${JCS[@]}" ping >/dev/null 2>&1; then return 0; fi
        sleep 0.1
    done
    fail "daemon never became ready; log: $(cat "$WORK/daemon.log")"
}

step "build"
cargo build --quiet --bins --manifest-path "$ROOT/Cargo.toml"
BIN="$ROOT/target/debug"
JCS=("$BIN/jcs" --socket "$WORK/jcsd.sock")

step "fixture repository"
mkdir -p "$WORK/repo" "$WORK/data"
git -C "$WORK/repo" init -q -b main
git -C "$WORK/repo" config user.email e2e@jcs.local
git -C "$WORK/repo" config user.name "E2E Fixture"
printf '# Fixture\n' > "$WORK/repo/README.md"
git -C "$WORK/repo" add -A
git -C "$WORK/repo" commit -qm "chore: initial commit"

export XDG_DATA_HOME="$WORK/data"
export JCS_DB="$WORK/jcs.db"
export JCS_SOCKET="$WORK/jcsd.sock"
export JCS_MAX_CONCURRENT=1
export RUST_LOG=warn

step "start daemon"
"$BIN/jcsd" > "$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
wait_for_daemon

step "doctor"
"${JCS[@]}" doctor > "$WORK/doctor.txt" 2>&1 || {
    cat "$WORK/doctor.txt"
    fail "doctor reported a problem"
}
grep -q "all checks passed" "$WORK/doctor.txt" || fail "doctor did not report success"

step "register project"
"${JCS[@]}" project add "$WORK/repo" --name e2e > /dev/null

step "ask"
"${JCS[@]}" ask -p e2e \
    "Create a file named HELLO.md whose only content is the exact text JCS_E2E_OK \
(no punctuation, no extra lines). Do not use shell commands." \
    --no-plan --model "$MODEL" > "$WORK/ask.txt" 2>&1 || {
    cat "$WORK/ask.txt"
    fail "ask did not succeed"
}
grep -q "run succeeded" "$WORK/ask.txt" || {
    cat "$WORK/ask.txt"
    fail "the run did not report success"
}

step "verify the commit"
BRANCH=$(git -C "$WORK/repo" worktree list --porcelain | awk '/^branch refs\/heads\/jcs/{sub("refs/heads/","",$2); print $2; exit}')
[ -n "$BRANCH" ] || fail "no jcs branch was created"

CHANGED=$(git -C "$WORK/repo" diff --name-only main..."$BRANCH")
echo "branch: $BRANCH"
echo "changed: $CHANGED"
echo "$CHANGED" | grep -qx "HELLO.md" || fail "HELLO.md is not part of the commit"

CONTENT=$(git -C "$WORK/repo" show "$BRANCH:HELLO.md" | tr -d '\r' | sed -e 's/[[:space:]]*$//')
[ "$CONTENT" = "JCS_E2E_OK" ] || fail "unexpected content: [$CONTENT]"

git -C "$WORK/repo" log --oneline "$BRANCH" | grep -qE "^[0-9a-f]+ feat\(e2e\): " \
    || fail "the commit message is not a conventional feat(e2e) commit"

step "verify the audit trail"
"${JCS[@]}" runs | grep -q "succeeded" || fail "no succeeded run in the run list"
"${JCS[@]}" logs --limit 200 | grep -q "gate worktree-clean: passed" \
    || fail "the worktree-clean gate is missing from the log"

printf '\nPASS: objective -> worktree -> agent -> gates -> commit\n'
