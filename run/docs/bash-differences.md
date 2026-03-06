# How Runfile differs from plain bash

Runfile syntax is designed to feel like bash, but there are a few important differences to be aware of.

## Parameters become `local` shell variables

Named parameters in a function signature are injected as `local` variable declarations inside the generated shell function. For example:

```bash
deploy(env, version = "latest") {
    echo "Deploying $version to $env"
}
```

generates roughly:

```bash
__run__() {
    local env="$1"
    local version="${2:-latest}"
    echo "Deploying $version to $env"
}
__run__ "$@"
```

Because bash handles the expansion natively, all standard bash variable features work:
- `${var:-default}` — default values
- `'$var'` — single quotes protect the variable from expansion
- `$env` does **not** accidentally match `$env_config` (no substring pollution)

Functions without named parameters still receive arguments as `$1`, `$2`, `$@` as usual.

## Block bodies are wrapped in `__run__()`

Block function bodies are wrapped in a shell function called `__run__()`. This means:
- `return` works as expected (exits the function, not the script)
- `exit` exits the subshell process

## `set -e` is not the default

Runfile does not inject `set -e` automatically. If you want strict error handling, add it at the top of your block:

```bash
build() {
    set -euo pipefail
    cargo fmt -- --check
    cargo clippy
    cargo test
}
```

## `source` is context-dependent

- **Top-level** `source` directives are expanded by `run` at parse time — they merge functions from another file.
- **Inside function bodies**, `source` is passed through to the shell interpreter as a normal shell `source` command.

## Namespace colons become double underscores

When function names contain colons (e.g., `docker:build`), the colons are rewritten to double underscores in the generated shell functions for compatibility. You can invoke them with either syntax: `run docker build` or `run docker:build`.

## `--show-script` for debugging

Use `run --show-script <function> [args...]` to print the exact script that would be passed to the shell, without executing it. This is useful for debugging parameter handling and preamble injection.
