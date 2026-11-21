#!/usr/bin/env bash
# End-to-end validation of policy-as-code with an executable gate.
#
# The proof is constructed so it cannot pass by luck: the objective asks for "a short
# greeting", while the gate requires a token no model would guess. The only way the run
# can end with that token in the file is if the gate failed, the repair prompt was
# delivered, and the agent changed its work in response.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
command -v cargo >/dev/null 2>&1 || export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo not found" >&2; exit 1; }

MODEL=${JCS_E2E_MODEL:-opencode-go/hy4-flash-free}
TOKEN=JCS_POLICY_GATE_OK
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

step "fixture pack with an executable gate"
mkdir -p "$WORK/packs/exact-marker"
cat > "$WORK/packs/exact-marker/pack.toml" <<EOF
id = "exact-marker"
version = "1.0.0"
description = "Fixture pack: MARKER.txt must contain an exact token."
instructions = ["instructions.md"]

[rules]
marker_token = "MARKER.txt holds one exact token provided by the gate output."

[gates.marker]
command = ["sh", "-c", 'if [ "\$(cat MARKER.txt 2>/dev/null)" = "$TOKEN" ]; then exit 0; fi; echo "MARKER.txt must contain exactly: $TOKEN"; exit 1']
timeout_secs = 20
parser = "generic"
EOF
printf '# Fixture instructions\n\nThe gate is the source of truth.\n' \
    > "$WORK/packs/exact-marker/instructions.md"

step "fixture repository with a project policy"
mkdir -p "$WORK/repo/.jcs" "$WORK/data"
git -C "$WORK/repo" init -q -b main
git -C "$WORK/repo" config user.email policy@jcs.local
git -C "$WORK/repo" config user.name "Policy Fixture"
printf '# Fixture\n' > "$WORK/repo/README.md"
git -C "$WORK/repo" add -A
git -C "$WORK/repo" commit -qm "chore: initial commit"
printf 'packs = ["exact-marker"]\n' > "$WORK/repo/.jcs/policy.toml"

export XDG_DATA_HOME="$WORK/data"
export JCS_DB="$WORK/jcs.db"
export JCS_SOCKET="$WORK/jcsd.sock"
export JCS_PACKS_DIR="$WORK/packs"
export JCS_DEFAULT_PACKS=""
export JCS_MAX_REPAIR_CYCLES=2
export JCS_MAX_CONCURRENT=1
export RUST_LOG=warn

step "start daemon"
"$BIN/jcsd" > "$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 1 100); do
    if "${JCS[@]}" ping >/dev/null 2>&1; then break; fi
    sleep 0.1
done
"${JCS[@]}" ping >/dev/null 2>&1 || fail "daemon never became ready"

"${JCS[@]}" project add "$WORK/repo" --name policy > /dev/null

step "the policy knows the custom pack"
"${JCS[@]}" policy show policy > "$WORK/policy.txt"
grep -q "exact-marker 1.0.0" "$WORK/policy.txt" || {
    cat "$WORK/policy.txt"
    fail "the custom pack is not part of the policy"
}
grep -q "marker_token" "$WORK/policy.txt" || fail "the pack rule is missing"
grep -q "marker" "$WORK/policy.txt" || fail "the gate is missing"

step "render is deterministic and check passes afterwards"
"${JCS[@]}" policy render policy > "$WORK/render.txt"
grep -q "created .*AGENTS.md" "$WORK/render.txt" || {
    cat "$WORK/render.txt"
    fail "AGENTS.md was not created"
}
"${JCS[@]}" policy render policy --check > /dev/null || fail "check failed right after render"

HEAD_BEFORE=$(sha256sum "$WORK/repo/AGENTS.md" | cut -d' ' -f1)
"${JCS[@]}" policy render policy > /dev/null
HEAD_AFTER=$(sha256sum "$WORK/repo/AGENTS.md" | cut -d' ' -f1)
[ "$HEAD_BEFORE" = "$HEAD_AFTER" ] || fail "rendering twice changed the output"

step "the gate fails before any work exists"
if "${JCS[@]}" gates run policy > "$WORK/gates-before.txt" 2>&1; then
    cat "$WORK/gates-before.txt"
    fail "the marker gate should fail on an empty repository"
fi
grep -q "must contain exactly" "$WORK/gates-before.txt" || {
    cat "$WORK/gates-before.txt"
    fail "the gate output is not actionable"
}

step "ask: the agent must satisfy the gate, not the wording of the objective"
"${JCS[@]}" ask -p policy "Create a file MARKER.txt containing a short greeting." \
    --no-plan --model "$MODEL" > "$WORK/ask.txt" 2>&1 || {
    cat "$WORK/ask.txt"
    fail "the run did not succeed"
}
grep -q "run succeeded" "$WORK/ask.txt" || {
    cat "$WORK/ask.txt"
    fail "the run did not report success"
}

step "verify the repair loop actually ran"
BRANCH=$(git -C "$WORK/repo" worktree list --porcelain | awk '/^branch refs\/heads\/jcs/{sub("refs/heads/","",$2); print $2; exit}')
[ -n "$BRANCH" ] || fail "no task branch was created"

CONTENT=$(git -C "$WORK/repo" show "$BRANCH:MARKER.txt" | tr -d '\r' | sed -e 's/[[:space:]]*$//')
[ "$CONTENT" = "$TOKEN" ] || fail "expected the gate token, got [$CONTENT]"

grep -q "asking the agent to repair" "$WORK/daemon.log" || {
    tail -20 "$WORK/daemon.log"
    fail "the repair loop never ran; the token must have been produced by luck"
}

step "the run recorded the gate verdict"
"${JCS[@]}" runs | grep -q "succeeded" || fail "the run is not recorded as succeeded"

printf '\nPASS: policy -> gate -> failure -> repair -> green, with an exact match\n'
