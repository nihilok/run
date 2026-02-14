# Reference

Quick lookups for attributes, environment variables, and discovery rules.

## Attribute summary
- `@desc <text>` — short description for listings and MCP tools.
- `@arg <name> [type] <description>` — document parameters (names should match the signature). Optional type can be `string`, `integer`, `float`/`number`, `boolean`, or `object`/`dict`.
- `@os <unix|windows|macos|linux>` — restrict a function to a platform.
- Platform branching: use separate `# @os` variants or branch inside the shell body (inline `@macos {}` style guards are not supported).
- `@shell <interpreter>` — force an interpreter (`python3`, `node`, `pwsh`, `bash`, `sh`, etc.). Overrides any shebang.

## Runfile discovery and precedence
1. `--working-dir / --runfile` if provided (no merging).
2. Otherwise, project `Runfile` is merged with global `~/.runfile` (project definitions override globals).
3. Set `RUN_NO_GLOBAL_MERGE=1` to disable merging and use only the project Runfile (or fall back to global if no project file exists).

## Environment variables
- `RUN_SHELL` — default shell when no interpreter is specified. Defaults to `sh` on Unix/macOS, `pwsh` (or `powershell`) on Windows.
- `RUN_MCP_OUTPUT_DIR` — directory for MCP output files when responses are truncated.
- `RUN_NO_GLOBAL_MERGE` — skip merging `~/.runfile` into the project Runfile (useful for isolation/tests).

## Output handling
- `--output-format stream|json|markdown` controls how results are emitted.
- Structured output is used when a function returns it (e.g., MCP-aware functions); otherwise output is streamed.

## Interpreters
Supported interpreters include `python`, `python3`, `node`, `ruby`, `pwsh`, `bash`, and `sh`. Use a shebang or `@shell` to select one; `@shell` wins if both are present.
