# run

[![Crates.io](https://img.shields.io/crates/v/run.svg)](https://crates.io/crates/run)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

**a.k.a. runtool: the bridge between human and AI tooling**

Define functions in a `Runfile`, run them instantly from your terminal, or expose them as MCP tools for AI agents. Shell, Python, Node—whatever fits the task.

## Quick start
```bash
# Runfile

# @desc Deploy to an environment
# @arg env Target environment (staging|prod)
# @arg version Version to deploy (defaults to "latest")
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env..."
    ./scripts/deploy.sh $env $version
}
```

```bash
run deploy staging
run deploy prod v2.1.0
```

## Install
- Homebrew: `brew install nihilok/tap/runtool`
- Scoop: `scoop bucket add nihilok https://github.com/nihilok/scoop-bucket` then `scoop install runtool`
- Cargo: `cargo install run` (or `runtool`)

## Documentation
- [Getting started](https://github.com/nihilok/run-rust/blob/main/docs/getting-started.md)
- [Runfile syntax](https://github.com/nihilok/run-rust/blob/main/docs/runfile-syntax.md), [arguments](https://github.com/nihilok/run-rust/blob/main/docs/arguments.md), and [variables](https://github.com/nihilok/run-rust/blob/main/docs/variables.md)
- [Attributes and interpreters](https://github.com/nihilok/run-rust/blob/main/docs/attributes-and-interpreters.md)
- [Polyglot commands](https://github.com/nihilok/run-rust/blob/main/docs/polyglot-commands.md) and [command composition](https://github.com/nihilok/run-rust/blob/main/docs/command-composition.md)
- [CLI usage](https://github.com/nihilok/run-rust/blob/main/docs/cli.md)
- [MCP integration](https://github.com/nihilok/run-rust/blob/main/docs/mcp.md)
- [Recipes](https://github.com/nihilok/run-rust/blob/main/docs/recipes.md)
- [Reference](https://github.com/nihilok/run-rust/blob/main/docs/reference.md) and [FAQ](https://github.com/nihilok/run-rust/blob/main/docs/faq.md)

## License
MIT
