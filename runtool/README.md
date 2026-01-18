# runtool

[![Crates.io](https://img.shields.io/crates/v/runtool.svg)](https://crates.io/crates/runtool)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/nihilok/run/blob/main/LICENSE)

**The bridge between human and AI tooling**

Define functions in a `Runfile`. Your AI agent discovers and executes them via the built-in MCP server. You run them from the terminal too with instant startup and tab completion. Shell, Python, Node—whatever fits the task.

```bash
# @desc Search the codebase for a pattern
# @shell python
search(pattern: str) {
    import sys, os, re
    for root, _, files in os.walk('.'):
        for f in files:
            path = os.path.join(root, f)
            try:
                for i, line in enumerate(open(path), 1):
                    if re.search(sys.argv[1], line):
                        print(f"{path}:{i}: {line.rstrip()}")
            except: pass
}
```

Humans run it from the terminal:
```bash
$ run search "TODO"
```

AI agents discover and execute it automatically via the Model Context Protocol (MCP).

---

**Note:** `runtool` is an alternative package name for `run`. Both install the same `run` binary and provide identical functionality.

## Installation

```bash
cargo install runtool
```

This installs the `run` binary to your cargo bin directory.

### Recommended Installation

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

## Quick Start

Create a `Runfile` in your project:

```bash
# @desc Deploy to an environment
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env..."
    ./scripts/deploy.sh $env $version
}

# @desc Run the development server
dev() {
    cargo run --release
}
```

Run from terminal:
```bash
$ run deploy staging
$ run dev
```

Enable AI integration:
```bash
$ run --serve-mcp
```

## Full Documentation

See the [run crate documentation](https://crates.io/crates/run) for:
- AI Agent Integration (MCP)
- Complete syntax guide
- Function composition
- Polyglot scripts (Python, Node, Ruby, etc.)
- Tab completions
- Platform-specific functions

## Source Code

Both `run` and `runtool` are maintained in the same repository:
https://github.com/nihilok/run
