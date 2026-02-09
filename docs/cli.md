# CLI usage

Most commands follow `run <function> [args...]`. The CLI also offers discovery, completions, structured output, and an MCP server mode.

## Common commands
- Call a function: `run deploy staging v1.2.3`
- List available functions: `run --list`
- Execute a script file directly: `run ./script.run`
- Start the interactive REPL (no args): `run`

## Flags
- `--list` — print all callable functions in the current Runfile.
- `--inspect` — output the MCP JSON schema for all functions (descriptions, parameters, defaults).
- `--serve-mcp` — start the MCP server so AI agents can call your functions.
- `--working-dir PATH` (alias `--runfile`) — point `run` at a specific project directory.
- `--output-format stream|json|markdown` — choose how results are emitted; `json`/`markdown` use structured output when supported by the function.
- `--install-completion [SHELL]` — install shell completions (auto-detects if omitted).
- `--generate-completion SHELL` — print completion script without installing.

## Output formats
- `stream` (default): stream stdout/stderr directly.
- `json`: emit structured results when a function returns them (falls back to streamed output otherwise).
- `markdown`: format structured results for MCP/AI-friendly rendering.

## Completions
```bash
run --install-completion       # detects shell
run --generate-completion zsh  # print script for manual install
```
Supports `bash`, `zsh`, `fish`, and `powershell`.

## Working with multiple Runfiles
`run` searches upward from the current directory. Use `--working-dir` to target a different project, or create a `~/.runfile` for global utilities that are searched after the local Runfile.
