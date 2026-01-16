# run

[![Crates.io](https://img.shields.io/crates/v/run.svg)](https://crates.io/crates/run)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

**Lightweight task runner that speaks shell, Python, Node, and more, that communicates natively with your AI Agents.**

Define tasks in a `Runfile`, run them from anywhere. Make them available to AI via the in-built MCP server.

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
  - [Function Composition](#function-composition)
  - [AI Agent Integration (MCP)](#ai-agent-integration-mcp)
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
- **vs Both**: `run` is the only task runner with built-in Model Context Protocol support, letting AI agents like Claude discover and execute your tools automatically.

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

### Function Composition

Functions can call other functions defined in the same Runfile, enabling task composition and code reuse without duplication.

```bash
# Base tasks
build() cargo build --release
test() cargo test
lint() cargo clippy

# Composed task that calls other functions
ci() {
    echo "Running CI pipeline..."
    lint
    test
    build
}

# Deploy depends on successful build
deploy() {
    build || exit 1
    echo "Deploying..."
    scp target/release/app server:/bin/
}
```

When you run `run ci`, all compatible functions are automatically injected into the execution scope, so you can call them directly without spawning new processes.

**Key features:**
- Functions can call sibling functions defined in the same file
- Exit codes are properly propagated (use `|| exit 1` to stop on failure)
- Works across different shells when interpreters are compatible
- Top-level variables are also available to all functions

---

## AI Agent Integration (MCP)

`run` includes built-in support for the **Model Context Protocol (MCP)**, allowing AI agents like Claude to discover and execute your Runfile functions as tools.

### Exposing Functions to AI Agents

Use `@desc` and `@arg` attributes to provide metadata for AI agents:

```bash
# @desc Search the codebase for specific patterns
# @arg 1:pattern string The regex pattern to search for
# @shell python
search() {
    import sys, os, re
    pattern = sys.argv[1]
    for root, dirs, files in os.walk('.'):
        for file in files:
            if file.endswith('.py'):
                path = os.path.join(root, file)
                with open(path) as f:
                    for i, line in enumerate(f, 1):
                        if re.search(pattern, line):
                            print(f"{path}:{i}: {line.strip()}")
}

# @desc Deploy the application to a specific environment
# @arg 1:environment string Target environment (staging|prod)
deploy() {
    ./scripts/deploy.sh $1
}
```

### MCP Server Mode

Start `run` as an MCP server to enable AI agent integration:

```bash
run --serve-mcp
```

Configure in your AI client (e.g., Claude Desktop):

```json
{
  "mcpServers": {
    "my-project": {
      "command": "run",
      "args": ["--serve-mcp", "--runfile", "/path/to/your/project/Runfile"],
      "cwd": "/path/to/your/project"
    }
  }
}
```

**Note**: The `--runfile` argument is required to specify which Runfile the AI agent should use. This allows you to expose specific Runfiles to different AI contexts.

Now AI agents can:
- Discover available tools via `run --inspect`
- Execute functions with typed parameters
- Receive structured outputs

### Inspect Tool Schema

View the generated JSON schema for all MCP-enabled functions:

```bash
run --inspect
```

This outputs the tool definitions that AI agents will see, useful for debugging and validation.

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