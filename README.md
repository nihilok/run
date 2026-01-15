# run

[![Crates.io](https://img.shields.io/crates/v/run.svg)](https://crates.io/crates/run)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

**Task runner that speaks shell, Python, Node, and Ruby.**

Define tasks in a `Runfile`, run them from anywhere. No TOML, no YAML, no new syntax to learn.

```bash
# Runfile

build() cargo build --release

# @os windows
clean() del /Q dist

# @os unix
clean() rm -rf dist

# @shell python
analyze() {
    import sys, json
    with open(sys.argv[1]) as f:
        data = json.load(f)
        print(f"Found {len(data)} records")
}

# @shell node
server() {
    require('http').createServer((req, res) => {
        res.end('Hello from Runfile!');
    }).listen(process.argv[1] || 3000);
}
```

```bash
$ run build
$ run analyze data.json
$ run server 8080
```

Perfect for **project automation**, **CI scripts**, and **personal workflows**.

---

## Table of Contents

- [Why run?](#why-run)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [The Runfile Syntax](#the-runfile-syntax)
  - [Basic Functions](#basic-functions)
  - [Block Syntax](#block-syntax)
  - [Arguments & Defaults](#arguments--defaults)
  - [Attributes & Polyglot Scripts](#attributes--polyglot-scripts)
  - [Nested Namespaces](#nested-namespaces)
- [Configuration](#configuration)
  - [Shell Selection](#shell-selection)
  - [Global Runfile](#global-runfile)
- [License](#license)

---

## Why run?

It hits a common sweet spot — lightweight, readable, and shell-native for quick CLI automation without the overhead of heavier task systems.

- **Zero Config**: Shell, Python, Node, or Ruby: right in your Runfile.
- **Low Overhead**: Instant startup time.
- **Shell Native**: Use the syntax you already know (`$1`, `&&`, pipes).
- **Clean Namespace**: Organise tasks with `group:task` syntax.
- **Global & Local**: Project-specific `./Runfile` or personal `~/.runfile`.

### Comparison

- **vs Make**: `run` is easier for linear scripts and doesn't require learning Makefile quirks (tabs vs spaces, `.PHONY`).
- **vs Just**: `run` is closer to raw shell scripting. It doesn't have a custom language for variables or logic—it just delegates to your shell.

---

## Installation

### Recommended

**macOS/Linux (Homebrew)**
```bash
brew tap nihilok/tap
brew install runfile
```

**Windows (Scoop)**
```powershell
scoop bucket add nihilok https://github.com/nihilok/scoop-bucket
scoop install runfile
```

### Alternative: Cargo

Works on all platforms:

```bash
cargo install run
```

### Tab Completions

Auto-detect your shell and install completions:

```bash
run --install-completion
```

Supports `bash`, `zsh`, `fish`, and `powershell`.

---

## Quick Start

Create a `Runfile` in your project root:

```bash
# Simple one-liner
dev() cargo run

# Multi-step task
deploy() {
    echo "Building..."
    cargo build --release
    echo "Deploying..."
    scp target/release/app server:/bin/
}

# Use Python for complex logic
# @shell python
stats() {
    import sys
    lines = sum(1 for line in open(sys.argv[1]))
    print(f"{sys.argv[1]}: {lines} lines")
}
```

Run your tasks:

```bash
$ run dev
$ run deploy
$ run stats src/main.rs
```

List available tasks:

```bash
$ run --list
```

---

## The Runfile Syntax

`run` parses your `Runfile` to find function definitions. The syntax is designed to be familiar to anyone who has used `bash` or `sh`.

### Basic Functions

For simple, one-line commands, you don't need braces.

```bash
# Usage: run dev
dev() cargo run

# Usage: run fmt
fmt() cargo fmt
```

### Block Syntax

Use `{}` for multi-statement functions. This avoids the need for trailing backslashes.

```bash
ci() {
    echo "Running CI..."
    cargo fmt -- --check
    cargo clippy
    cargo test
    echo "Done!"
}
```

### Arguments & Defaults

Arguments are passed directly to the underlying shell. Access them using standard positional variables: `$1`, `$2`, `$@`.

```bash
# Usage: run commit "Initial commit"
git:commit() {
    git add .
    git commit -m "$1"
}

# Usage: run deploy prod v2
# $1 = prod, $2 = v2 (defaults to 'latest' if missing)
deploy() {
    env=$1
    version=${2:-latest} 
    echo "Deploying $version to $env..."
}

# Pass all arguments through
test() {
    # Usage: run test --release --nocapture
    cargo test $@
}
```

### Attributes & Polyglot Scripts

You can use comment attributes (`# @key value`) or shebang lines to modify function behaviour and select interpreters.

#### Platform Guards (`@os`)

Restrict functions to specific operating systems. This allows you to define platform-specific implementations of the same task.

```bash
# @os windows
clean() del /Q dist

# @os unix
clean() rm -rf dist
```

When you run `run clean`, only the variant matching your current OS will execute.

#### Interpreter Selection

There are two ways to specify a custom interpreter:

**1. Shebang detection** (recommended):

The first line of your function body can be a shebang, just like standalone scripts:

```python
analyze() {
    #!/usr/bin/env python
    import sys, json
    with open(sys.argv[1]) as f:
        data = json.load(f)
        print(f"Found {len(data)} records")
}
```

```javascript
server() {
    #!/usr/bin/env node
    const port = process.argv[1] || 3000;
    require('http').createServer((req, res) => {
        res.end('Hello!');
    }).listen(port);
}
```

**2. Attribute syntax** (`@shell`):

Use comment attributes for explicit control or when you need to override a shebang:

```python
# @shell python3
calc() {
    import sys, math
    radius = float(sys.argv[1])
    print(f"Area: {math.pi * radius**2:.2f}")
}
```

**Precedence**: If both are present, `@shell` takes precedence over the shebang.

**Supported interpreters:** `python`, `python3`, `node`, `ruby`, `pwsh`, `bash`, `sh`

### Nested Namespaces

Organise related commands using colons. `run` parses `name:subname` as a single identifier.

```bash
docker:build() docker build -t app .
docker:up() docker compose up -d
docker:logs() docker compose logs -f
```

Execute them with spaces:

```bash
$ run docker build
$ run docker logs
```

---

## Configuration

### Shell Selection

By default, `run` uses:

- **Windows**: PowerShell (`pwsh` if available, else `powershell`)
- **Unix**: `sh`

You can override this default by setting the `RUN_SHELL` environment variable.

```bash
# Force Zsh for this command
RUN_SHELL=zsh run build

# Make it permanent for your session
export RUN_SHELL=bash
```

**Note**: The commands in your Runfile must be compatible with the configured shell, unless an explicit interpreter (e.g., `# @shell python`) is defined for that function.

### Global Runfile

Create a `~/.runfile` in your home directory to define global commands available anywhere.

```bash
# ~/.runfile

# Usage: run update
update() {
    brew update
    brew upgrade
    rustup update
}

# Usage: run clone <repo>
clone() {
    git clone "https://github.com/$1"
    cd "$(basename "$1" .git)"
}
```

If a local `./Runfile` exists, `run` looks there first. If the command isn't found locally, it falls back to `~/.runfile`.

---

## License

MIT