# run

[![Crates.io](https://img.shields.io/crates/v/run.svg)](https://crates.io/crates/run)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

**a.k.a. runtool: the bridge between human and AI tooling**

Define functions in a `Runfile`. Your AI agent discovers and executes them via the built-in MCP server. You run them from the terminal too with instant startup and tab completion. Shell, Python, Node—whatever fits the task.

This project is a specialized fork of `sporto/run-rust`, optimized specifically for Model Context Protocol (MCP) integration, enabling automatic tool discovery for AI agents.

```bash
# Runfile

# @desc Search the codebase for a pattern
# @shell python
# @arg pattern The regex pattern to search for
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

# @desc Deploy to an environment
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env..."
    ./scripts/deploy.sh $env $version
}

# @desc Analyze a JSON file
function analyze(file: str) {
    #!/usr/bin/env python3
    import sys, json
    with open(sys.argv[1]) as f:
        data = json.load(f)
        print(f"Found {len(data)} records")
}
```

_The syntax is designed to be similar to bash/sh, while being permissive & flexible, with added features for AI integration._

Humans can run these functions directly from the terminal:

```bash
$ run search "TODO"
$ run deploy staging
$ run analyze data.json
```

Point your AI agent at the Runfile, and it can discover and execute these tools automatically.

---

## Table of Contents

- [AI Agent Integration (MCP)](#ai-agent-integration-mcp)
- [Installation](#installation)
- [The Runfile Syntax](#the-runfile-syntax)
  - [Basic Functions](#basic-functions)
  - [Block Syntax](#block-syntax)
  - [Function Signatures](#function-signatures)
  - [Syntax Guide](#syntax-guide)
  - [Attributes & Polyglot Scripts](#attributes--polyglot-scripts)
  - [Nested Namespaces](#nested-namespaces)
  - [Function Composition](#function-composition)
- [Recipes](#recipes)
- [Configuration](#configuration)
- [License](#license)

---

## AI Agent Integration (MCP)

`run` has built-in support for the **Model Context Protocol (MCP)**, allowing AI agents like Claude to discover and execute your Runfile functions as tools.

### MCP Server Mode

Configure in your AI client. For **Claude Desktop**, add this configuration to:
- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "my-project": {
      "command": "run",
      "args": ["--serve-mcp"],
      "env": {
        "PWD": "/path/to/your/project/"
      }
    }
  }
}
```

`run` automatically discovers your `Runfile` by searching up from the current working directory. You can optionally use `--runfile` (alias: `--working-dir`) to specify an explicit path. MCP output files (`.run-output`) are written next to your Runfile. To override the output location explicitly, set `RUN_MCP_OUTPUT_DIR` in the environment.

Now your AI agent can discover and call your tools automatically.

For debugging purposes, you can start `run` as an MCP server yourself:

```bash
run --serve-mcp
```

### Describing Tools for AI

Use `@desc` to describe what a function does, and declare parameters in the function signature:

```bash
# @desc Search the codebase for a regex pattern
search(pattern: str) {
    #!/usr/bin/env python3
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

# @desc Deploy the application to a specific environment
deploy(environment: str, version = "latest") {
    ./scripts/deploy.sh $environment $version
}
```

Functions with `@desc` are automatically exposed as MCP tools with typed parameters.

### Inspect Tool Schema

View the generated JSON schema for all MCP-enabled functions:

```bash
run --inspect
```

This outputs the tool definitions that AI agents will see—useful for debugging and validation.

### Built-in MCP Tools

In addition to your Runfile functions, `run` provides built-in tools for managing execution context:

- **`set_cwd`** - Change the current working directory. Useful for multi-project workflows where the agent needs to switch between different project contexts.
  ```
  set_cwd(path: string)
  ```

- **`get_cwd`** - Get the current working directory. Helps agents understand their current execution context.
  ```
  get_cwd()
  ```

These tools allow AI agents to navigate your filesystem and work with multiple projects in a single MCP session.

---

## Installation

### Recommended

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

### Alternative: Cargo

Works on all platforms:

```bash
cargo install run  # or: cargo install runtool
```

### Tab Completions

Auto-detect your shell and install completions:

```bash
run --install-completion
```

Supports `bash`, `zsh`, `fish`, and `powershell`.

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

### Function Signatures

Declare parameters directly in the function signature for cleaner, self-documenting code:

```bash
# @desc Deploy to an environment
deploy(env: str, version = "latest") {
    echo "Deploying $version to $env..."
    ./scripts/deploy.sh $env $version
}

# @desc Resize an image
resize(width: int, height: int, file: str) {
    convert $file -resize ${width}x${height} output.png
}
```

**Type annotations** (`str`, `int`, `bool`) are used for:
- MCP JSON schema generation (AI agents see typed parameters)
- Self-documenting functions
- Optional runtime validation

**Default values** make parameters optional:

```bash
# version defaults to "latest" if not provided
deploy(env: str, version = "latest") { ... }
```

**Legacy positional syntax** still works for simple cases:

```bash
# Access arguments as $1, $2, $@
deploy() {
    env=$1
    version=${2:-latest}
    ./scripts/deploy.sh $env $version
}
```

**Combining with `@arg` for descriptions:**

```bash
# @desc Deploy the application
# @arg env Target environment (staging|prod)
# @arg version Version tag to deploy
deploy(env: str, version = "latest") {
    ./scripts/deploy.sh $env $version
}
```

When both signature params and `@arg` exist, the signature defines names/types/defaults, and `@arg` provides descriptions for MCP.

### Syntax Guide

`run` allows you to embed Python, Node.js, or other interpreted languages directly inside shell functions using **shebang detection**. When `run` encounters a shebang line (e.g., `#!/usr/bin/env python`) as the first line of a function body, it automatically:

1. **Detects the interpreter** from the shebang path
2. **Extracts the function body** (excluding the shebang line itself)
3. **Executes the content** using that interpreter

**Key behaviors:**

- **Argument passing**: Shell arguments (like `$1`, `$2`) are passed to the script as command-line arguments. In Python, access them via `sys.argv[1]`, `sys.argv[2]`, etc. In Node.js, use `process.argv[2]`, `process.argv[3]`, etc.
- **Function signature integration**: If your function declares typed parameters (e.g., `analyze(file: str)`), these are still accessed positionally in the embedded script.
- **Shebang precedence**: If both a shebang and `@shell` attribute are present, `@shell` takes precedence.

**Example:**

```bash
# @desc Analyze a JSON file
analyze(file: str) {
    #!/usr/bin/env python3
    import sys, json
    # $file becomes sys.argv[1]
    with open(sys.argv[1]) as f:
        data = json.load(f)
        print(f"Found {len(data)} records")
}
```

When you run `run analyze data.json`, `run` detects the Python shebang and executes the function body as a Python script with `data.json` passed as `sys.argv[1]`.

This polyglot approach lets you mix shell orchestration with specialized scripting languages in a single `Runfile` without needing separate script files.

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

You can also use inline platform guards within a single function to define OS-aware implementations:

```bash
# @desc Run the build process (OS-aware)
build() {
  @linux { ./scripts/build_linux.sh }
  @macos { ./scripts/build_macos.sh }
  @windows { .\scripts\build_win.ps1 }
}
```

`run` evaluates these guards at runtime and executes only the block matching your current platform.

#### Interpreter Selection

There are two ways to specify a custom interpreter:

**1. Shebang detection**

The first line of your function body can be a shebang, just like standalone scripts:

```
analyze() {
    #!/usr/bin/env python
    import sys, json
    with open(sys.argv[1]) as f:
        data = json.load(f)
        print(f"Found {len(data)} records")
}
```

```
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

## Recipes

Here are practical examples demonstrating real-world DevOps workflows.

### Testing, Deployment & Secure Database Access

This example shows how to structure a `Runfile` for a typical application lifecycle. An important security feature: **AI agents only see function descriptions and parameters via MCP, never the implementation details**—so secrets in your function bodies remain hidden.

```bash
# @desc Run all unit tests
test_unit() {
  pytest tests/unit
}

# @desc Safely run a read-only query against a database
# @arg query The SQL query to execute
# @arg env The target environment (local|prod)
db_query(query: str, env: str) {
  #!/usr/bin/env python3
  import sys, os, psycopg2

  query = sys.argv[1]
  target_env = sys.argv[2]

  # The AI can't see this implementation.
  # Connection strings can be hardcoded here or loaded from environment.
  db_url = os.getenv(f"DB_CONNECTION_{target_env.upper()}")
  # Or: db_url = "postgresql://user:pass@localhost/mydb"

  if not db_url:
      print(f"Error: Connection not configured for {target_env}.")
      sys.exit(1)

  # Connect and execute (simplified example)
  print(f"Executing query on {target_env}...")
  conn = psycopg2.connect(db_url)
  cur = conn.cursor()
  cur.execute(query)
  for row in cur.fetchall():
      print(row)
  cur.close()
  conn.close()
}

# @desc Deploy to production or staging
# @arg env The target environment (staging|prod)
deploy(env: str) {
  echo "Deploying to $env..."
  # Implementation details hidden from AI
  ./scripts/deploy.sh $env
}
```

**Security Benefit**: When you run `run --inspect`, the AI sees:
```json
{
  "name": "db_query",
  "description": "Safely run a read-only query against a database",
  "inputSchema": {
    "properties": {
      "query": {"type": "string"},
      "env": {"type": "string"}
    }
  }
}
```

The function body—including any credentials, API keys, or implementation logic—is never exposed to the AI agent. This allows you to safely hardcode secrets or reference them from your environment without risk of leakage.

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

### MCP Output Files

When running in MCP mode, outputs longer than 50 lines are truncated and the full text is saved under a `.run-output` directory next to your Runfile. The directory is created automatically. To override the location (e.g., for sandboxing), set `RUN_MCP_OUTPUT_DIR` before starting the server:

```bash
RUN_MCP_OUTPUT_DIR=/tmp/run-output run --serve-mcp
```

---

## License

MIT