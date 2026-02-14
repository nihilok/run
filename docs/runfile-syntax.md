# Runfile syntax

A `Runfile` is a catalog of callable functions. `run` looks for `Runfile` in the working directory (or parent directories) and injects its functions into an execution scope.

## Function forms
- **Inline:** concise single-line commands
  ```bash
  dev() cargo run
  fmt() cargo fmt
  ```
- **Block:** multi-line bodies without trailing backslashes
  ```bash
  ci() {
      echo "Running CI..."
      cargo fmt -- --check
      cargo clippy
      cargo test
  }
  ```
- **`function` keyword:** optional; both `build()` and `function build()` are accepted.

## Signatures
Declare parameters in the function header. They become shell variables during execution.

```bash
# @desc Deploy to an environment
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env"
}
```

- Parameters are positional; the order in the signature matches CLI arguments.
- Defaults make parameters optional.
- A rest parameter captures the remaining arguments:
  ```bash
  echo_all(...args) echo "Args: $args"
  ```
  - When forwarding `...args`, use `cmd $args` to expand into separate tokens; `cmd "$args"` collapses them into a single argument. If you need to preserve the original token boundaries (including spaces) safely, prefer `cmd "$@"` instead—for example, `cargo test --package run "$@"`.
- You can still reference legacy positional tokens (`$1`, `$2`) alongside named params.
- Named parameters work in polyglot functions too (Python, Node.js, Ruby). `run` auto-injects variables so you can use `name` directly instead of `sys.argv[1]`. See [Polyglot commands](./polyglot-commands.md).

See [Arguments](./arguments.md) for mapping/defaults and [Variables](./variables.md) for scope and environment details.

## Namespaces
Use colons to group related tasks. Invoke with spaces or colons.

```bash
docker:build() docker build -t app .
docker:logs(service = "app") docker compose logs -f $service
```

Run as `run docker build` or `run docker:build`.

## Comments and attributes
Lines beginning with `#` can hold human comments or attributes (e.g., `# @desc`). Attributes adjust behavior and metadata; see [Attributes and interpreters](./attributes-and-interpreters.md).
