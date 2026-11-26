---
key: T00002
kind: task
title: "Parser: handle '#' in ${...} parameter expansions and fix parse error locations"
status: backlog
depends_on: []
covers: []
touches:
  - "run/src/grammar.pest"
  - "run/src/config.rs"
  - "run/src/parser/mod.rs"
  - "run/src/parser/preprocessing.rs"
  - "run/src/parser/error.rs"
  - "run/src/executor.rs"
  - "run/src/mcp/tools.rs"
  - "run/src/mcp/mapping.rs"
  - "run/tests/basic_functions.rs"
test_commands:
  - "cargo test --workspace"
  - "cargo clippy --workspace --all-targets"
---

## Context

GitHub issue [#47](https://github.com/nihilok/run/issues/47) (`nihilok/run#47`) has three related root causes:

1. **Grammar (`run/src/grammar.pest`)**: `block_part` matches `comment_in_block = @{ "#" ~ (!NL ~ ANY)* }` unconditionally anywhere inside a block, including inside nested `{ ... }` from shell parameter expansions (`${v##*:}`, `${v#*:}`, `${#v}`). When `#` appears inside `${...}`, `comment_in_block` swallows the closing `}` and the rest of the line, unbalancing braces and failing with `expected block body (\`{ ... }\`)`.
2. **Brace scanner (`run/src/config.rs`)**: `count_unquoted_braces` breaks on any unquoted `'#'` (`'#' => break`), so `${v##*:}` or `${#v}` causes `brace_depth` to become unbalanced, corrupting subsequent top-level `source` and `# @instructions` scanning.
3. **Misleading parse error locations** (e.g., `Runfile:206:9` pointing at `# @desc` in a 6-line file):
   - `parse_script` runs `preprocess_escaped_newlines(input)` (collapsing `\`-continued lines and reducing line counts) before calling `ScriptParser::parse`, while callers pass the un-preprocessed source to `ParseError::from_pest`, shifting line numbers and printing the wrong source line.
   - `load_merged_config()` concatenates `~/.runfile` + `\n` + `./Runfile` into one string, while callers (`executor.rs`, `mcp/tools.rs`, `mcp/mapping.rs`) report filename `"Runfile"` with the merged line number instead of mapping back to `~/.runfile` or `./Runfile` with file-local line numbers.

Intended to be implemented in its own separate commit (independent of #46).

## Acceptance criteria

- [ ] Block function bodies containing shell parameter expansions with `#` — including `${v##*:}`, `${v#*:}`, `${#v}`, nested expansions like `${v:-${other#*:}}`, and quoted values inside `${...}` — parse and execute with expected shell output (e.g. `v=host:5432; echo ${v##*:}` prints `5432`; `${#v}` prints `9`).
- [ ] Non-comment `#` in shell expressions inside blocks (such as `$#` and mid-word `foo#bar`) does not swallow closing braces or following syntax, while real comments containing unbalanced quotes/apostrophes (`# It's a comment`) and shebangs (`#!/usr/bin/env python3`) continue to parse without regression (#32).
- [ ] `config::count_unquoted_braces` does not treat `#` inside `${...}` (or non-comment `#`) as a comment terminator, so `expand_source_directives` and `collect_mcp_instructions` keep accurate `brace_depth` across functions using `${v##*:}` / `${#v}`.
- [ ] Parse errors in files containing backslash-escaped newlines (`\`) before the error report the accurate 1-indexed line number and matching `source_line` from the original (un-preprocessed) input.
- [ ] Parse errors occurring in merged `~/.runfile` + `./Runfile` configs attribute the error to the correct source (`~/.runfile` vs `Runfile`) with that file's 1-indexed line number and source line rather than a combined merged offset.
- [ ] Unit and integration tests cover all `${...}` `#` forms (parse + execution), `#32` apostrophe comments regression, `count_unquoted_braces` with `${v##*:}`/`${#v}`, escaped-newline error line accuracy, and merged-runfile error attribution; `cargo test --workspace` and `cargo clippy --workspace --all-targets` pass cleanly.

## Decisions log

- Scope includes both the `#` lexing fix (grammar + `config::count_unquoted_braces`) AND the parse error location attribution fixes (escaped-newline line mapping + merged global/project file & line attribution), as confirmed with the user.
- In `run/src/grammar.pest`, match `${...}` parameter expansions explicitly (allowing nested braces, quoted strings, escapes, and `#` without triggering `comment_in_block`) and restrict `comment_in_block` so `#` only begins a comment at comment-valid positions (preserving `#32` apostrophe-in-comment handling).
- Implement in a separate commit from #46.

## Failure modes

- Regressing `#32`: unbalanced single/double quotes inside real shell/Python comments (`# It's fine`, `echo ok # don't break`) must still be absorbed by `comment_in_block`.
- Failing on nested `${...}` expansions (e.g. `${a:-${b##*/}}`), quotes inside `${...}` (e.g. `${v#*"#"}`), or escaped `}` (`\}`) inside `${...}`.
- Breaking `parse_attributes_from_lines` line lookup (`original_input[..span.start()].lines().count()`) if preprocessing changes line structure unexpectedly.
- Off-by-one errors when mapping merged global+project line numbers across the `\n` separator (especially when global `~/.runfile` does or does not end with a trailing newline).
- Breaking shebangs (`#!/usr/bin/env ...`) at the start of block bodies.

## Open questions

- None.

## Verification
