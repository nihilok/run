# Command composition

Compose functions to build richer workflows without duplicating logic.

## Calling other functions
Call sibling functions directly; they are injected into the execution scope.
```bash
build() cargo build --release
test() cargo test
lint() cargo clippy

ci() {
    echo "Running CI..."
    lint || exit 1
    test || exit 1
    build
}
```

Key behaviors:
- Exit codes propagate; guard dependent steps with `|| exit 1` when needed.
- Top-level Runfile variables are visible to all functions.

## Cross-language patterns
- Use shell functions to orchestrate calls into language-specific helpers.
- Combine platform guards with composition to select the right implementation per OS.
- Keep shared setup (env vars, temp dirs) in one function and reuse it across tasks.

For interpreter mixing patterns, see [Polyglot commands](./polyglot-commands.md).
