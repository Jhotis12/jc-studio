#!/usr/bin/env python3
"""Contract drift guard.

Verifies that the vendored OpenCode OpenAPI document is valid and still exposes
every operation `jcs-agent` depends on. Runs in CI without a live service.

Exit codes: 0 ok, 1 drift or malformed contract.
"""

from __future__ import annotations

import json
import pathlib
import sys

CONTRACT = pathlib.Path("docs/contracts/opencode-2.3.2.json")

# operationId -> required HTTP method
REQUIRED_OPERATIONS = {
    "server.info": "get",
    "session.create": "post",
    "session.prompt": "post",
    "session.get": "get",
    "session.message.list": "get",
    "session.diff": "get",
    "session.interrupt": "post",
    "session.permission.list": "get",
    "session.permission.reply": "post",
    "event.subscribe": "get",
    "project.list": "get",
    "model.list": "get",
}

METHODS = {"get", "post", "put", "patch", "delete"}


def fail(msg: str) -> None:
    print(f"contract: FAIL - {msg}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    if not CONTRACT.exists():
        fail(f"missing contract file at {CONTRACT}")

    try:
        doc = json.loads(CONTRACT.read_text())
    except json.JSONDecodeError as exc:
        fail(f"contract is not valid JSON: {exc}")

    if not doc.get("openapi"):
        fail("contract has no `openapi` version field")

    found: dict[str, str] = {}
    for path, item in doc.get("paths", {}).items():
        for method, op in item.items():
            if method not in METHODS or not isinstance(op, dict):
                continue
            op_id = op.get("operationId")
            if op_id:
                found[op_id] = method

    missing = sorted(set(REQUIRED_OPERATIONS) - set(found))
    if missing:
        fail("missing required operations: " + ", ".join(missing))

    wrong = sorted(
        op_id
        for op_id, method in REQUIRED_OPERATIONS.items()
        if found.get(op_id) != method
    )
    if wrong:
        fail("operations changed HTTP method: " + ", ".join(wrong))

    security = doc.get("components", {}).get("securitySchemes", {})
    if not security:
        print("contract: warning - no securitySchemes declared")

    print(f"contract: OK - {len(found)} operations, {len(REQUIRED_OPERATIONS)} required present")


if __name__ == "__main__":
    main()
