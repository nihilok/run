# runtool

[![Crates.io](https://img.shields.io/crates/v/runtool.svg)](https://crates.io/crates/runtool)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/nihilok/run-rust/blob/main/LICENSE)

`runtool` installs the `run` binary—define tasks in a `Runfile`, run them from your terminal, or expose them to AI agents via MCP.

```bash
# Runfile example
# @desc Deploy to an environment
# @arg env Target environment (staging|prod)
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env..."
    ./scripts/deploy.sh $env $version
}
```

```bash
run deploy staging
run --serve-mcp   # expose functions to AI agents
```

## Install
- Cargo: `cargo install runtool`
- Homebrew: `brew install nihilok/tap/runtool`
- Scoop: `scoop bucket add nihilok https://github.com/nihilok/scoop-bucket && scoop install runtool`

## Docs
Full documentation and guides live in the main repository:
- Getting started: https://github.com/nihilok/run-rust/blob/main/docs/getting-started.md
- Runfile syntax and arguments: https://github.com/nihilok/run-rust/tree/main/docs

## Source
`run` and `runtool` share the same codebase: https://github.com/nihilok/run-rust
