# Attributes and interpreters

Attributes live in comments (`# @key value`) and adjust how a function is exposed or executed. Interpreter selection can be declared via attributes or shebangs.

## Descriptions and args
- `@desc` — one-line summary shown in listings and MCP tool schemas.
- `@arg <name> [type] <description>` — add human-readable parameter docs. Names should match the signature. Optional type keyword (`string`, `integer`, `float`/`number`, `boolean`, `object`/`dict`) sets the JSON schema type for MCP when the function has no typed signature.
- `@instructions <text>` — top-level MCP guidance line appended to server `initialize.instructions`. This is single-line and repeatable; lines are aggregated in merged/source order.

```bash
# @desc Deploy to an environment
# @arg env Target environment (staging|prod)
# @arg version Version to deploy (defaults to "latest")
deploy(env: str, version = "latest") { ... }
```

> **`@arg` type vs. signature type hint:** The type keyword in `@arg` and the type annotation in the function signature serve related but distinct purposes:
>
> - **Signature type hint** (e.g., `env: str`) is the primary driver. When a typed signature is present, it controls the MCP JSON schema type *and*, in polyglot functions (Python, Node.js, Ruby), drives automatic runtime value conversion.
> - **`@arg` type keyword** (e.g., `# @arg env string …`) is a fallback used when the function has no typed signature — for example, shell functions that rely on positional variables (`$1`, `$2`). In that case, the `@arg` type sets the MCP schema type.
> - **`@arg` description** is always used regardless of whether a signature type hint is present.
> - The two do not need to agree, but keeping them consistent is recommended. If they conflict, the signature type hint wins for MCP schema generation.

Top-level MCP instruction example:

```bash
# @instructions Confirm target environment before deploy calls
# @instructions Prefer exact keywords when using memory recall queries
```

## Platform guards
Limit a function to specific operating systems.

**Attribute form (separate functions by OS):**
```bash
# @os windows
clean() del /Q dist

# @os unix
clean() rm -rf dist
```

## Interpreter selection
There are two ways to pick an interpreter for a function body:

1) **Shebang detection** — first line inside the body
```bash
script() {
    #!/usr/bin/env python3
    import sys
    print(sys.argv[1])
}
```

2) **`@shell` attribute** — explicit, and takes precedence over a shebang
```bash
# @shell node
serve() {
    console.log(process.argv[1] || 3000);
}
```

Supported interpreters include `python`, `python3`, `node`, `ruby`, `pwsh`, `bash`, and `sh`.

## Precedence and resolution
- `@shell` overrides a shebang if both exist.
- If no interpreter is set, the function uses the default shell (`RUN_SHELL` or platform default).

For language-specific behaviors and argument forwarding, see [Polyglot commands](./polyglot-commands.md).
