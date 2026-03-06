# run Documentation

The `run` tool executes functions defined in a `Runfile`, exposing them both to your terminal and to AI agents via the Model Context Protocol (MCP). It is built for fast startup, polyglot scripts, and discoverable tooling.

## Who this is for
- Developers who want a single, typed catalog of project tasks.
- AI users who need MCP-exposed tools without revealing implementation details.
- Teams that mix shell automation with Python/Node snippets in one file.

## How this documentation is organised
- [Getting started](./getting-started.md) — installation and your first Runfile.
- [Runfile syntax](./runfile-syntax.md) — functions, namespaces, and signatures.
- [Arguments](./arguments.md) — how parameters map to shell variables and defaults.
- [Variables](./variables.md) — environment handling and scope rules.
- [Attributes and interpreters](./attributes-and-interpreters.md) — `@desc`, `@os`, `@shell`, and shebang precedence.
- [Polyglot commands](./polyglot-commands.md) — mixing languages inside a Runfile.
- [Command composition](./command-composition.md) — combining tasks and propagating exit codes.
- [CLI usage](./cli.md) — flags, output formats, completions, and REPL.
- [MCP integration](./mcp.md) — exposing tools to AI agents safely.
- [Recipes](./recipes.md) — ready-made Runfile snippets.
- [Reference](./reference.md) — attribute and environment variable quick lookups.
- [FAQ](./faq.md) — fast answers to common questions.
