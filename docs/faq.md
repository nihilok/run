# FAQ

## How are arguments passed?
They follow the order in the function signature and become shell variables with matching names. Legacy `$1`, `$2`, `$@` still work. See [Arguments](./arguments.md) and [Variables](./variables.md).

## How do I choose an interpreter?
Add a shebang as the first line of the body or set `# @shell <interpreter>`. The attribute wins if both are present. Details in [Attributes and interpreters](./attributes-and-interpreters.md).

## Can I mix languages in one Runfile?
Yes. Each function can use its own interpreter. Use shell functions to orchestrate calls between Python/Node helpers; see [Polyglot commands](./polyglot-commands.md) and [Command composition](./command-composition.md).

## How do I document tools for AI agents?
Add `@desc` and `@arg` comments. Run `run --inspect` to confirm the generated schema. More in [MCP integration](./mcp.md).

## Where are Runfiles discovered?
`run` walks up from the current directory, then falls back to `~/.runfile`. Override with `--working-dir`. See the [reference](./reference.md).

## What if my output is very large?
In MCP mode, long output is truncated in the response and the full text is written to `.run-output/` (configurable via `RUN_MCP_OUTPUT_DIR`).

## How do I install shell completions?
Run `run --install-completion` (auto-detects shell) or `run --generate-completion SHELL` for manual setup. See [CLI usage](./cli.md).

## How do I run tasks from another project?
Use `run --working-dir /path/to/project <function> ...` so discovery happens in that directory.

## Are type hints enforced at runtime?
No. Types in signatures power documentation and MCP schemas. Validate inside your function if needed.

## Is it safe to keep secrets in functions when using MCP?
Theoretically, yes. MCP exposes only the schema (names, descriptions, parameter shapes). Function bodies are not sent to the agent. 
However, if secrets are in your project Runfile and agents have other tools to read files, there is nothing to stop them from reading 
the Runfile (as with a plaintext .env file). See [MCP integration](./mcp.md) for security notes.
