set shell := ["bash", "-uc"]
export PATH := env_var_or_default("HOME", "") + "/.cargo/bin:" + env_var_or_default("PATH", "")

default: check

# Show available recipes
list:
    @just --list

# Format the whole workspace
fmt:
    cargo fmt --all

# Verify formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Lint with warnings as errors
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run the test suite
test:
    cargo test --workspace --all-targets

# Full local gate: the same checks CI runs
check: fmt-check lint test
    @echo "all gates green"

# Build release binaries
build:
    cargo build --release

# Run the daemon in the foreground with debug logs
daemon:
    RUST_LOG=debug cargo run --bin jcsd -- --foreground

# Run the CLI against the local daemon
cli *args:
    cargo run --bin jcs -- {{args}}

# Opt-in end-to-end test against a real OpenCode service
e2e:
    ./scripts/e2e.sh

# Prove the gate -> repair loop with a gate the objective cannot satisfy by luck
e2e-policy:
    ./scripts/e2e-policy.sh

# Goal decomposition and DAG execution
e2e-plan:
    ./scripts/e2e-plan.sh

# DAG + integration branch + promotion into the base branch
e2e-merge:
    ./scripts/e2e-merge.sh

# Kill the daemon mid-run and verify honest recovery
chaos:
    ./scripts/chaos.sh

# Audit dependencies for licenses and advisories
audit:
    cargo deny check

# Refresh the vendored OpenAPI contract from the running service
contract:
    mkdir -p docs/contracts
    opencode api GET /openapi.json > docs/contracts/opencode-2.3.2.json
    @echo "contract refreshed"
