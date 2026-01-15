# RFC 003: AI Agent Support & Model Context Protocol (MCP)

**Status**: Draft | **Type**: Feature | **Target**: v0.3.0  
**Topic**: AI Tooling, MCP Integration, Metadata Reflection

## 1. Summary

This proposal enables `run` to function as an ad-hoc **Model Context Protocol (MCP) Server**.

By adding structured comments (doc-comments) to a Runfile, users can expose their shell functions as typed "Tools" to AI agents (Claude, ChatGPT, etc.). This effectively turns a simple Runfile into a secure, sandboxed, and polyglot tool manifest for local AI development.

---

## 2. Motivation

Currently, giving an AI agent access to local tools requires high friction: initializing an MCP TypeScript/Python project, writing boilerplate SDK code, and compiling.

`run` is uniquely positioned to solve this because:

- **It defines boundaries**: The AI can only execute what is explicitly defined in the Runfile (Sandboxing).
- **It is concise**: A 3-line shell function is easier to write than a 50-line TypeScript class.
- **It is polyglot**: With RFC 001 (Attributes), users can define tools in Python, Node, or Shell within a single file.

---

## 3. Syntax Specification: Metadata Tags

To allow LLMs to understand how to use a function, we introduce new **Attribute Tags** (`# @`) specifically for documentation and schema generation.

### 3.1. `@desc` (Description)

A natural language description of what the function does. This is passed directly to the LLM to help it decide when to call the tool.

**Syntax**: `# @desc <text>`

```bash
# @desc Restarts the docker containers and tails the logs
restart() { ... }
```

### 3.2. `@arg` (Argument Mapping)

Maps the AI's named parameters to the shell's positional arguments (`$1`, `$2`, etc.).

**Syntax**: `# @arg <position>:<name> [type] <description>`

- **position**: The shell argument index (1, 2, 3...).
- **name**: The key exposed in the JSON Schema (e.g., `service_name`).
- **type**: (Optional) JSON schema type (`string`, `integer`, `boolean`). Defaults to `string`.
- **description**: Help text for the LLM.

```bash
# @desc Scale a specific service
# @arg 1:service string The name of the docker service (e.g., 'web')
# @arg 2:replicas integer The number of instances to spin up
scale() {
    docker compose scale $1=$2
}
```

---

## 4. CLI Interface

### 4.1. `run --inspect` (Schema Dump)

Outputs the generated JSON schema for all functions in the Runfile. This is useful for debugging or static configuration (e.g., pasting into OpenAI Playground).

**Output format**:

```json
{
  "tools": [
    {
      "name": "scale",
      "description": "Scale a specific service",
      "input_schema": {
        "type": "object",
        "properties": {
          "service": { 
            "type": "string", 
            "description": "The name of the docker service" 
          },
          "replicas": { 
            "type": "integer", 
            "description": "The number of instances to spin up" 
          }
        },
        "required": ["service", "replicas"]
      }
    }
  ]
}
```

### 4.2. `run --serve-mcp` (Server Mode)

Starts a long-running process that communicates via **Stdio** using the **JSON-RPC 2.0** format specified by the Model Context Protocol.

- **Handshake**: Responds to initialization requests.
- **List Tools**: Returns the schema generated from `@desc` and `@arg` tags.
- **Call Tool**: Accepts a JSON object, maps the named parameters to positional arguments, executes the command, and captures stdout/stderr as the result.

---

## 5. Technical Implementation

### 5.1. Argument Mapping Logic (The Bridge)

When the MCP server receives a call:

```json
call_tool("scale", { "service": "api", "replicas": 3 })
```

`run` must:

1. Look up the `scale` function.
2. Parse the `@arg` tags.
3. Map `service` → `$1` and `replicas` → `$2`.
4. Construct the command vector: `["docker", "compose", "scale", "api=3"]` (or however the function body dictates).
5. Execute using the appropriate shell/interpreter (per RFC 001).

### 5.2. Structured Output

If the shell function outputs JSON, the MCP server should ideally attempt to parse it and return it as a generic content block to the AI, rather than just a raw string.

**Optimization**: If the function defines `# @shell python` or `# @shell node`, the AI is likely expecting data. We should ensure stdout is captured cleanly (not mixed with logging).

---

## 6. Security Considerations

This feature turns `run` into a gateway for AI agents.

- **Allowlist Only**: The AI cannot execute arbitrary shell commands. It can only execute functions explicitly defined in the Runfile.
- **ReadOnly Mode**: We might consider a `--readonly` flag that only exposes functions marked with a `# @safe` tag (future consideration).
- **Human in the Loop**: MCP clients usually ask for user permission before running a tool. `run` relies on the client (e.g., Claude Desktop, Cursor) for this permission layer.

---

## 7. Example Workflow

### The Runfile (`~/project/Runfile`):

```python
# @desc Search the codebase for specific patterns
# @arg 1:pattern string The regex pattern to search for
# @shell python
search() {
    import sys, os, re
    pattern = sys.argv[1]
    # ... python logic to walk dir and json.dump results ...
}

# @desc Deploy the application
# @arg 1:env string The target environment (staging|prod)
deploy() {
    ./scripts/deploy.sh $1
}
```

### The Agent Config (e.g., `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "my-project": {
      "command": "run",
      "args": ["--serve-mcp"],
      "cwd": "/Users/me/project"
    }
  }
}
```

### The Result:

The user asks Claude: **"Check the codebase for TODOs and then deploy to staging."**

1. Claude sees tools: `search`, `deploy`.
2. Claude calls `search(pattern="TODO")`. `run` executes the Python block.
3. Claude reviews output.
4. Claude calls `deploy(env="staging")`. `run` executes the shell script.