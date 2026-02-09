# Getting started

Follow these steps to install `run` and try your first `Runfile`.

## Install

**macOS/Linux (Homebrew)**
```bash
brew tap nihilok/tap
brew install runtool
```

**Windows (Scoop)**
```powershell
scoop bucket add nihilok https://github.com/nihilok/scoop-bucket
scoop install runtool
```

**All platforms (Cargo)**
```bash
cargo install run   # or: cargo install runtool
```

## Create your first Runfile

1) Create a file named `Runfile` in your project root.

```bash
# @desc Deploy to an environment
# @arg env Target environment (staging|prod)
# @arg version Version to deploy (defaults to "latest")
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env..."
    ./scripts/deploy.sh $env $version
}
```

2) Run it from your terminal:
```bash
run deploy staging
run deploy prod v2.1.0
```

3) List available functions to check discovery:
```bash
run --list
```

4) Install shell completions (auto-detects your shell):
```bash
run --install-completion
```

## Working directory
`run` searches upward from the current working directory for a `Runfile`. To point at another project explicitly, use `--working-dir /path/to/project`.

## Next steps
- Learn the [Runfile syntax](./runfile-syntax.md), [arguments](./arguments.md), and [variables](./variables.md).
- Explore [polyglot commands](./polyglot-commands.md) and [command composition](./command-composition.md).
