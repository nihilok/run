# run

A lightweight task runner for defining and executing shell commands with a clean, readable syntax. Define functions in a `Runfile` (or `~/.runfile`) and call them from the command line to streamline your development workflow.

[![Crates.io](https://img.shields.io/crates/v/run.svg)](https://crates.io/crates/run)
[![Docs.rs](https://docs.rs/run/badge.svg)](https://docs.rs/run)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

#### Why use `run`?

It hits a common sweet spot — lightweight, readable, and shell-native for quick CLI automation without the overhead of heavier task systems.

- Simple, familiar syntax for shell users and low onboarding cost.
- Block functions (`{}`) provide clean multi-statement definitions without shell escaping.
- Nested names, positional args (`$1`, `$@`) and default-value support cover most everyday tasks.
- Multi-line commands, variables, and REPL make iterative development fast.
- Global (`~/.runfile`) and project-specific (`./Runfile`) scopes.

### What are the alternatives?

- `make`: More about dependency tracking and rebuilding; heavyweight for simple command orchestration. `run` is easier for linear scripts and ad-hoc tasks.
- `just`: Closer in spirit (task runner with recipes). `just` has richer features (recipe interpolation, shebangs, some safety) while `run` is simpler and more shell-native.
- Plain shell scripts: More flexible but less discoverable and reusable. `run` provides a structured, listable command surface.
- Language-based task runners (e.g., npm scripts, Mage): Offer ecosystem hooks and richer logic; `run` is lighter and language-agnostic.

## Prerequisites

- [Homebrew package manager (macOS/Linux)](https://brew.sh/), [Scoop package manager (Windows)](https://scoop.sh/), OR [Rust toolchain with Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
- **Windows users:** PowerShell is used by default (pwsh or powershell). You can override this by setting the `RUN_SHELL` environment variable to use other shells like Git Bash, WSL, or MSYS2.

## Installation

### macOS / Linux

Install via Homebrew:

```sh
brew tap nihilok/tap
brew install runfile
```

### Windows

Install via Scoop:

```powershell
scoop bucket add nihilok https://github.com/nihilok/scoop-bucket
scoop install runfile
```

OR

Install via crates.io with Cargo:

```
cargo install run
```

### Tab Completions

After installation, enable tab completions for your shell:

```
run --install-completion  # Auto-detects your shell (bash/zsh/fish)
run --install-completion bash  # Or specify explicitly
```

Or generate completion scripts manually:

```sh
run --generate-completion bash > ~/.local/share/bash-completion/completions/run
run --generate-completion zsh > ~/.zsh/completion/_run
run --generate-completion fish > ~/.config/fish/completions/run.fish
```

## Features

- **Simple Function Definitions:** Define reusable functions in a `Runfile` with clean syntax
- **Block Functions:** Use `{}` braces for multi-statement functions with cleaner syntax
- **Nested Functions:** Organise related commands with colon notation (e.g., `docker:shell`, `python:test`)
- **Argument Passing:** Pass arguments to functions using `$1`, `$2`, `$@`, etc.
- **Default Values:** Set fallback values using bash-style syntax (e.g., `${2:-default}`)
- **Multi-line Commands:** Chain commands with `&&` and split across lines with `\`
- **Variable Support:** Define and use variables in your scripts
- **Interactive REPL:** Start an interactive shell for testing commands
- **List Functions:** Quickly view all available functions with `--list`
- **Global or Project-Specific:** Use `~/.runfile` for global commands or `./Runfile` for project-specific ones

## Quick Start

Create a `Runfile` in your project root:

```runfile
# Build and run commands
build() cargo build --release
test() cargo test
dev() cargo run

# Multi-statement functions using blocks
ci() {
    echo "Running CI pipeline..."
    cargo fmt -- --check
    cargo clippy
    cargo test
    echo "All checks passed!"
}

# Docker commands with arguments
docker:shell() docker compose exec $1 bash
docker:logs() docker compose logs -f $1

# Git helpers with multiple arguments
git:commit() git add . && git commit -m "$1" && echo "${2:-Done}"
```

Run your functions:

```sh
run build
run docker shell web
run git commit "Initial commit" "All set!"
```

## Usage

### Basic Commands

Call a function from your Runfile:
```sh
run build
run test
run lint
```

### Passing Arguments

Functions can accept arguments which are available as `$1`, `$2`, `$@`, etc:
```sh
run docker shell app
run git commit "Fix bug" "Completed"
run deploy production us-east-1
```

### Nested Functions

Organise related commands with colon notation and call them with spaces:
```sh
run python test      # Calls python:test()
run docker shell web # Calls docker:shell() with "web" as $1
run node dev         # Calls node:dev()
```

### List Available Functions

View all functions defined in your Runfile:
```sh
run --list
run -l
```

### Run a Script File

Execute a standalone script file:
```sh
run myscript.run
```

### Interactive Mode

Start a REPL to test commands interactively:
```sh
run
```

## Runfile Examples

### Python (with uv)
```runfile
python:install() uv venv && uv pip install -r requirements.txt
python:test() uv run pytest
python:lint() uv run ruff check .
python:format() uv run black .

# Block function for complete CI
python:ci() {
    echo "Running Python CI..."
    uv run ruff check .
    uv run black --check .
    uv run pytest --cov
    echo "✓ All checks passed!"
}
```

### Node.js
```runfile
node:install() npm install
node:dev() npm run dev
node:build() npm run build
node:lint() npm run lint
node:test() npm test
```

### Docker
```runfile
docker:build() docker build -t myapp .
docker:run() docker run -it --rm myapp
docker:shell() docker compose exec $1 bash
docker:logs() docker compose logs -f $1
docker:up() docker compose up -d
docker:down() docker compose down

# Deploy with multiple steps
docker:deploy() {
    echo "Building image..."
    docker build -t myapp:$1 .
    echo "Pushing to registry..."
    docker push myapp:$1
    echo "Restarting containers..."
    docker compose up -d
    echo "✓ Deployed version $1"
}
```

### Git Helpers
```runfile
git:commit() git add . && git commit -m "$1" && echo "${2:-Done}"
git:amend() git commit --amend --no-edit
git:push() git push origin $(git branch --show-current)
```

### Block Functions

Use `{}` braces for cleaner multi-statement functions:

```runfile
# Multi-line block (newline separated)
deploy() {
    echo "Building..."
    cargo build --release
    echo "Testing..."
    cargo test
    echo "Deploying to $1..."
    scp target/release/app server:/app/
}

# Inline block (semicolon separated)
quick() { echo "Starting..."; cargo check; echo "Done!" }

# Traditional backslash continuation (still supported)
deploy_old() echo "Deploying to $1..." \
    && cargo build --release \
    && scp target/release/app server:/app/ \
    && echo "Deploy complete!"
```

### Using All Arguments
```runfile
echo_all() echo "All args: $@"
forward() docker exec myapp $@
```

## Runfile Syntax

- **Function Definition:** `name() command` or `name() command1 && command2`
- **Block Functions:** `name() { command1; command2 }` or multi-line with newlines
- **Nested Functions:** `category:name() command`
- **Arguments:** Access with `$1`, `$2`, `$3`, etc. or `$@` for all arguments
- **Default Values:** Use bash syntax like `${1:-default_value}`
- **Multi-line:** End lines with `\` to continue on the next line, or use `{}` blocks
- **Comments:** Lines starting with `#` are comments
- **Variables:** Define with `name=value` and use with `$name`

## Configuration

Place your `Runfile` in one of these locations:
- `./Runfile` - Project-specific commands (checked first)
- `~/.runfile` - Global commands available everywhere

Functions are executed in the underlying shell, so you can use any standard shell syntax, pipes, redirects, etc.

### Shell Selection

By default, `run` uses:
- **Windows:** PowerShell (`pwsh` if available, otherwise `powershell`)
- **Unix-like systems:** `sh`

You can override this by setting the `RUN_SHELL` environment variable:

```sh
# Use a different shell temporarily
RUN_SHELL=zsh run build

# Set it for your session (bash/zsh)
export RUN_SHELL=bash

# Set it for your session (PowerShell)
$env:RUN_SHELL = "bash"

# Windows users can use other shells like Git Bash
$env:RUN_SHELL = "bash"
run build

# Or use cmd (syntax in Runfile must be cmd-compatible)
set RUN_SHELL=cmd
run build
```

This allows you to use any shell you prefer, as long as it supports the `-c` flag for executing commands.

## License

MIT
