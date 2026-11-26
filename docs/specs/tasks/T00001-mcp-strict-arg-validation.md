---
key: T00001
kind: task
title: "MCP: reject missing required and undeclared arguments and tighten inputSchema"
status: done
depends_on: []
covers: []
touches:
  - "run/src/mcp/mapping.rs"
  - "run/src/mcp/tools.rs"
  - "run/src/mcp/handlers.rs"
  - "run/src/mcp/mod.rs"
  - "run/tests/mcp_server_test.rs"
  - "run/tests/mcp_tool_schema_test.rs"
test_commands:
  - "cargo test --workspace"
  - "cargo clippy --workspace --all-targets"
---

## Context

GitHub issue [#46](https://github.com/nihilok/run/issues/46) (`nihilok/run#46`) is **not yet fixed** on `main`:

- `map_arguments_to_positional` in `run/src/mcp/mapping.rs` initializes `vec![String::new(); max_position]` and fills omitted required arguments with `""` instead of rejecting the call.
- Undeclared argument keys in `tools/call` `arguments` are silently ignored rather than rejected, and `map_arguments_to_positional` returns early (`Ok(Vec::new())`) when `arg_mapping.is_empty()` without checking for unexpected keys or non-object `arguments`.
- `InputSchema` in `run/src/mcp/tools.rs` does not emit `"additionalProperties": false`, so MCP clients cannot validate unknown keys before calling.
- `run/src/mcp/mapping.rs` stores raw `arg_meta.name` without stripping trailing `?` via `sanitise_property_key`, creating a mismatch with `run/src/mcp/tools.rs` for optional legacy `@arg`s such as `2:branch?`.

Intended to be implemented in its own separate commit (independent of #47).

## Acceptance criteria

- [x] Calling a Runfile-backed MCP tool (`tools/call`) while omitting any required argument (non-optional `@arg` without `?` when `params` is empty, or signature `Parameter` with `default_value: None && !is_rest`) fails with JSON-RPC error code `-32602` naming the missing argument(s), and does not execute the function.
- [x] Passing JSON `null` for a required argument fails with JSON-RPC error code `-32602` naming the missing argument(s), and does not execute the function.
- [x] Calling a Runfile-backed MCP tool with any argument key not declared for that tool (excluding the built-in `timeout` parameter handled in `handlers.rs`) fails with JSON-RPC error code `-32602` naming the unexpected argument(s), and does not execute the function — including for zero-argument functions.
- [x] Valid optional arguments continue to work: signature params with default values, rest params (`...args`), legacy `@arg`s with `?` suffix (matched by sanitised key without `?`), and the built-in `timeout` parameter (non-negative integer or `null`).
- [x] Non-object `arguments` values return JSON-RPC error `-32602` ("Arguments must be an object"), including on zero-argument tools.
- [x] `InputSchema` serializes `"additionalProperties": false` in both `--inspect` and `tools/list` responses for all MCP tools (Runfile-derived and built-in).
- [x] Unit and integration tests cover missing required args, `null` required args, unknown args (with params, legacy `@arg`, and zero-arg tools), optional/`?`/default/rest/`timeout` args, non-object args, and `additionalProperties: false` schema output; `cargo test --workspace` and `cargo clippy --workspace --all-targets` pass with no warnings (`unwrap_used`/`expect_used` denied).

## Decisions log

- Return JSON-RPC `-32602` (Invalid params) naming missing/unknown arguments rather than `isError: true`, matching existing `handle_tools_call` validation for unknown tool, non-object arguments, and invalid `timeout`.
- Add `"additionalProperties": false` to `InputSchema` (`additional_properties: bool` with `#[serde(rename = "additionalProperties")]`) so MCP clients can catch mistakes pre-call.
- Promote `tools::sanitise_property_key` to `pub(super)` and reuse it in `mapping.rs` so legacy `@arg` key names and `?` optionality are interpreted identically by schema generation and argument validation.
- Implement in a separate commit from #47.

## Failure modes

- Forgetting to validate unknown keys when `arg_mapping.is_empty()` (zero-arg functions) or on the `rest_param` early-return branch in `map_arguments_to_positional`.
- Breaking valid calls that pass `timeout` (must be stripped in `handlers.rs` before unknown-key validation).
- Mismatch between sanitised property keys in `tools.rs` (e.g. `branch?` -> `branch`, invalid chars replaced with `_`) and keys checked in `mapping.rs`.
- Rejecting omitted optional params (`default_value.is_some()`, `is_rest`, or `?`-suffixed `@arg`) or treating `null` on an optional param as an error.
- Breaking existing tests that deserialize `InputSchema` without `additionalProperties` if `#[serde(default)]` is omitted.

## Open questions

- None.

## Verification

- `cargo fmt --all -- --check`: PASS (exit 0)
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS (exit 0, 0 warnings)
- `cargo test --workspace`: PASS (exit 0; 632 tests passed, 0 failed, 0 ignored — 373 unit tests in `run`, 259 integration tests across all 14 integration test suites)
