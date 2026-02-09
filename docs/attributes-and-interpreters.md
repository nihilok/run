# Attributes and interpreters

Attributes live in comments (`# @key value`) and adjust how a function is exposed or executed. Interpreter selection can be declared via attributes or shebangs.

## Descriptions and args
- `@desc` — one-line summary shown in listings and MCP tool schemas.
- `@arg <name> <description>` — add human-readable parameter docs. Names should match the signature.

```bash
# @desc Deploy to an environment
# @arg env Target environment (staging|prod)
# @arg version Version to deploy (defaults to "latest")
deploy(env: str, version = "latest") { ... }
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
