# Runfile syntax

A `Runfile` is a catalog of callable functions. `run` looks for `Runfile` in the working directory (or parent directories) and injects its functions into an execution scope.

## Function forms
- **Inline:** concise single-line commands
  ```bash
  dev() cargo run
  fmt() cargo fmt
  build-release() cargo build --release
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

## Block bodies: shell, quoting, and braces
- Bodies run in `bash -c` (falls back to `sh`; PowerShell on Windows), or `RUN_SHELL` if set. `@shell` or a shebang overrides this; see [Attributes and interpreters](./attributes-and-interpreters.md).
- `run` only scans a block to find its closing `}`. The body text goes to the shell verbatim, so normal shell quoting rules apply inside it.
- Braces inside `'…'` or `"…"` are ignored by the scanner, and so are braces in `#` comments. Balanced braces anywhere are fine. GraphQL, jq filters and inline JSON work directly in a block:
  ```bash
  issue(id: str) {
      q='{ "query": "query { issue(id: \"'"$id"'\") { title } }" }'
      curl -s -d "$q" "$API" | jq '.data.issue | {title}'
  }
  ```
- Unbalanced braces outside quotes break parsing. Common causes are a lone `{` in a heredoc or an unquoted apostrophe (`echo don't`), and the error is `expected block body`. Quote the text, or move that logic into a script.

## Signatures
Declare parameters in the function header. They become shell variables during execution.

```bash
# @desc Deploy to an environment
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env"
}
```

- Named parameters become `local` shell variables inside the function. All standard bash variable features (`${var:-default}`, single-quote protection, etc.) work as expected.
- Parameters are positional; the order in the signature matches CLI arguments.
- Defaults make parameters optional.
- A rest parameter captures the remaining arguments:
  ```bash
  echo_all(...args) echo "Args: $args"
  ```
  - When forwarding `...args`, use `cmd $args` to expand into separate tokens; `cmd "$args"` collapses them into a single argument. If you need to preserve the original token boundaries (including spaces) safely, prefer `cmd "$@"` instead—for example, `cargo test --package run "$@"`.
- Type annotations are optional: `param: type`. Supported types: `str`/`string`, `int`/`integer`, `float`/`number`, `bool`/`boolean`, `object`/`obj`/`dict`. Types drive MCP schema generation and, in polyglot functions, automatic value conversion.
- Annotations and defaults combine: `limit: int = 15`. An untyped default that is an integer (`limit = 15`) or `true`/`false` is advertised to MCP as `integer`/`boolean`, but its runtime value stays a string. Add an annotation if a polyglot function needs a real number or boolean.
- You can still reference legacy positional tokens (`$1`, `$2`) alongside named params.
- Named parameters work in polyglot functions too (Python, Node.js, Ruby). `run` auto-injects variables so you can use `name` directly instead of `sys.argv[1]`. See [Polyglot commands](./polyglot-commands.md).

See [Arguments](./arguments.md) for mapping/defaults and [Variables](./variables.md) for scope and environment details.

## Naming
Function names must start with a letter or `_` and can contain letters, digits, `_`, `:`, or `-`. Hyphens are transparently mapped to underscores in the generated shell script, so `build-release` and `build_release` both work and call the same function.

## Namespaces
Use colons to group related tasks. Invoke with spaces or colons.

```bash
docker:build() docker build -t app .
docker:logs(service = "app") docker compose logs -f $service
```

Run as `run docker build` or `run docker:build`.

## Sourcing other files

The `source` directive merges functions from another file into the current Runfile:

```bash
source ./shared.run
source ~/.runfiles/helpers.run
```

- Paths can be relative (resolved from the Runfile's directory), absolute, or use `~/` for the home directory.
- Quoted paths are supported: `source "path with spaces.run"`.
- Sourced files can themselves contain `source` directives (circular references are detected and skipped).
- If a sourced file doesn't exist, a warning is printed to stderr and execution continues.
- Functions defined later in the file override those from sourced files, so you can import a base set and selectively replace individual commands.
- `source` directives inside function bodies are passed through to the shell interpreter as normal shell `source` commands—only top-level directives are expanded by `run`.

```bash
# Import shared helpers, override one locally
source ./team-defaults.run

deploy() {
    echo "Custom deploy for this project"
}
```

## Comments and attributes
Lines beginning with `#` can hold human comments or attributes (e.g., `# @desc`). Attributes adjust behavior and metadata; see [Attributes and interpreters](./attributes-and-interpreters.md).
