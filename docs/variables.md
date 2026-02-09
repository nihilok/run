# Variables

Learn how variables are resolved when functions execute, including environment inheritance and Runfile-level scope.

## Environment variables
Functions inherit the caller environment. Two key variables affect `run` itself:
- `RUN_SHELL` — override the default shell for execution. Defaults to `sh` on Unix/macOS, `pwsh` (or `powershell`) on Windows.
- `RUN_MCP_OUTPUT_DIR` — directory for MCP output files when responses are truncated.

## Runfile scope
- Top-level variables declared in a Runfile are visible to all functions.
- Sibling functions are injected into the execution scope, so you can call them by name.

## Parameter variables
- Signature parameters become shell variables with matching names once the function starts.
- Legacy positional tokens (`$1`, `$2`, `$@`) remain available for backward compatibility.

## Polyglot interpreters
When a function uses a shebang or `@shell` interpreter, arguments are forwarded positionally into that interpreter:
- Python: `sys.argv[1]`, `sys.argv[2]`, ...
- Node.js: `process.argv[2]`, `process.argv[3]`, ...
- Other interpreters receive the same argv array; defaults are applied before forwarding.

For interpreter selection rules, see [Attributes and interpreters](./attributes-and-interpreters.md). For parameter behaviors, see [Arguments](./arguments.md).
