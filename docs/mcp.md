# MCP integration

`run` ships with a Model Context Protocol (MCP) server so AI agents can discover and invoke your Runfile functions as tools.

## Why it works well
- Functions stay in your Runfile; only names, descriptions, and parameters are exposed to the agent.
- Typed signatures produce JSON schemas automatically.
- Output files are saved when responses are large, preventing context overrun while keeping full data accessible.

## Start the MCP server
```bash
run --serve-mcp
```
This searches for the nearest `Runfile` (or `~/.runfile` fallback) and exposes its functions.

## Inspect the tool schema
```bash
run --inspect
```
Outputs the JSON schema that agents receive—useful for debugging descriptions, parameter types, and defaults.

## Configure MCP servers for your agents
Add an entry to your MCP config:
```json
{
  "mcpServers": {
    "runtool": {
      "command": "run",
      "args": ["--serve-mcp"]
    }
  }
}
```

## Built-in MCP tools
Alongside your Runfile functions, two helpers are always available:
- `set_cwd(path: string)` — change the working directory for subsequent calls.
- `get_cwd()` — report the current working directory.

## Output files and truncation
- Long outputs are truncated in the MCP response after 32 lines; the full text is saved to `.run-output/` next to your Runfile.
- Override the output location with `RUN_MCP_OUTPUT_DIR` if you need a different directory.

## Describing tools for agents
- Always include `@desc` and `@arg` comments so the schema is clear.
- Keep function names action-oriented (e.g., `deploy`, `db:query`, `docs:build`).
- Use defaults for optional inputs so agents can call tools with fewer arguments.

## Security notes
- Agents see only the schema, never the function body. Secrets embedded in functions are not exposed via MCP.
- Use platform guards (`@os`) to avoid serving tools that cannot run on the host, or use polyglot node/python scripts (`@shell`).
