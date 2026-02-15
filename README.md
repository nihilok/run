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
- AUR: `yay -S runtool` (or `paru -S runtool`)
- Cargo: `cargo install run` (or `runtool`)

## Documentation
- [Getting started](https://runtool.dev/docs/getting-started.html)
- [Runfile syntax](https://runtool.dev/docs/runfile-syntax.html), [arguments](https://runtool.dev/docs/arguments.html), and [variables](https://runtool.dev/docs/variables.html)
- [Attributes and interpreters](https://runtool.dev/docs/attributes-and-interpreters.html)
- [Polyglot commands](https://runtool.dev/docs/polyglot-commands.html) and [command composition](https://runtool.dev/docs/command-composition.html)
- [CLI usage](https://runtool.dev/docs/cli.html)
- [MCP integration](https://runtool.dev/docs/mcp.html)
- [Recipes](https://runtool.dev/docs/recipes.html)
- [Reference](https://runtool.dev/docs/reference.html) and [FAQ](https://runtool.dev/docs/faq.html)

## License
MIT
