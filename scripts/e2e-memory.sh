#!/usr/bin/env bash
# End-to-end validation of code memory.
#
# Proves that the index is built from real files, search returns citable results, the index
# is incremental, and the context broker actually reaches the agent prompt.
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
mkdir -p "$WORK/repo/src" "$WORK/data"
git -C "$WORK/repo" init -q -b main
git -C "$WORK/repo" config user.email memory@jcs.local
git -C "$WORK/repo" config user.name "Memory Fixture"

cat > "$WORK/repo/src/auth.rs" <<'EOF'
//! Email validation for the registration endpoint.

pub struct Validator;

impl Validator {
    /// Rejects addresses without a domain.
    pub fn validate_email(&self, value: &str) -> bool {
        value.contains('@')
    }
}
EOF
cat > "$WORK/repo/src/store.rs" <<'EOF'
//! Persistent storage.

pub struct Store {
    pub path: String,
}

impl Store {
    /// Opens the database, creating it when missing.
    pub fn open(path: &str) -> Store {
        Store { path: path.to_string() }
    }
}
EOF
printf '# Fixture\n' > "$WORK/repo/README.md"
git -C "$WORK/repo" add -A
git -C "$WORK/repo" commit -qm "chore: initial commit"

export XDG_DATA_HOME="$WORK/data"
export JCS_DB="$WORK/jcs.db"
export JCS_SOCKET="$WORK/jcsd.sock"
export JCS_MAX_CONCURRENT=1
export RUST_LOG=info

step "start daemon"
"$BIN/jcsd" > "$WORK/daemon.log" 2>&1 &
DAEMON_PID=$!
for _ in $(seq 1 100); do
    if "${JCS[@]}" ping >/dev/null 2>&1; then break; fi
    sleep 0.1
done
"${JCS[@]}" ping >/dev/null 2>&1 || fail "daemon never became ready"

"${JCS[@]}" project add "$WORK/repo" --name memory > /dev/null

step "registering the project indexes it in the background"
FILES=0
for _ in $(seq 1 40); do
    "${JCS[@]}" memory memory --json > "$WORK/stats0.json" 2>/dev/null || true
    FILES=$(python3 -c "
import json
try:
    print(json.load(open('$WORK/stats0.json'))['files'])
except Exception:
    print(0)
")
    [ "$FILES" -ge 2 ] && break
    sleep 0.5
done
[ "$FILES" -ge 2 ] || fail "the project was not auto-indexed (files=$FILES)"

step "explicit force index parses everything"
"${JCS[@]}" index memory --force > "$WORK/index.txt"
cat "$WORK/index.txt"
grep -qE "indexed 2 files" "$WORK/index.txt" || fail "expected two indexed files"

step "search by symbol returns a citation"
"${JCS[@]}" search -p memory "Validator" > "$WORK/search.txt"
cat "$WORK/search.txt"
grep -q "src/auth.rs:" "$WORK/search.txt" || fail "no citation for the symbol search"

step "search by comment words works without embeddings"
"${JCS[@]}" search -p memory "registration endpoint" > "$WORK/search2.txt"
grep -q "src/auth.rs:" "$WORK/search2.txt" || fail "comment words did not find the file"

step "stats report the index"
"${JCS[@]}" memory memory > "$WORK/stats.txt"
cat "$WORK/stats.txt"
grep -q "semantic search off" "$WORK/stats.txt" || fail "semantic search should be reported as off"
grep -qE "files    2 " "$WORK/stats.txt" || fail "stats should report two files"

step "the index is incremental"
printf '\n// touched\n' >> "$WORK/repo/src/auth.rs"
"${JCS[@]}" index memory > "$WORK/index2.txt"
cat "$WORK/index2.txt"
grep -qE "indexed 1 files \(1 unchanged" "$WORK/index2.txt" \
    || fail "only the changed file should have been re-indexed"

step "the context broker reaches the task prompt"
"${JCS[@]}" ask -p memory --no-plan \
    "Rename validate_email to is_valid_email and keep the behaviour identical." \
    --model "$MODEL" > "$WORK/ask.txt" 2>&1 || {
    cat "$WORK/ask.txt"
    fail "the task did not succeed"
}
grep -q "run succeeded" "$WORK/ask.txt" || {
    cat "$WORK/ask.txt"
    fail "the run did not report success"
}
grep -q "injecting code context" "$WORK/daemon.log" \
    || fail "no code context was injected into the agent prompt"

printf '\nPASS: index -> search -> incremental -> cited context in a real task\n'
