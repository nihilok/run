# run

[![Crates.io](https://img.shields.io/crates/v/run.svg)](https://crates.io/crates/run)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

**a.k.a. runtool: the bridge between human and AI tooling**

Define functions in a `Runfile`. Your AI agent discovers and executes them via the built-in MCP server. You run them from the terminal too with instant startup and tab completion. Shell, Python, Node—whatever fits the task.

This project is a specialized fork of `sporto/run-rust`, optimized specifically for Model Context Protocol (MCP) integration, enabling automatic tool discovery for AI agents.

### Quick Start

Here's a simple example:

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

Run it from your terminal:

```bash
$ run deploy staging
$ run deploy prod v2.1.0
```

Or point your AI agent at the Runfile, and it can discover and execute this tool automatically.

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
- [Configuration](#configuration)
  - [Environment Variables](#environment-variables)
  - [Global Runfile](#global-runfile)
  - [MCP Output Files](#mcp-output-files)
- [Real-World Examples](#real-world-examples)
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

`run` automatically discovers your `Runfile` by searching up from the current working directory. MCP output files (`.run-output`) are written next to your Runfile. To override the output location explicitly, set `RUN_MCP_OUTPUT_DIR` in the environment.

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

## Configuration

### Environment Variables

`run` supports the following environment variables to customize its behavior:

#### `RUN_SHELL`

Override the default shell interpreter for executing functions.

**Default behavior:**
- **Unix/Linux/macOS**: `sh`
- **Windows**: `pwsh` (if available), otherwise `powershell`

**Usage:**
```bash
# One-time override
RUN_SHELL=zsh run build

# Set for your session
export RUN_SHELL=bash
```

**Note:** Commands in your Runfile must be compatible with the configured shell, unless an explicit interpreter (e.g., `# @shell python`) is defined for that function.

#### `RUN_MCP_OUTPUT_DIR`

Specify where to write output files when running in MCP mode.

**Default behavior:**
- Outputs are written to `.run-output/` directory next to your Runfile
- If the Runfile location cannot be determined, falls back to system temp directory

**Usage:**
```bash
# Set output directory
RUN_MCP_OUTPUT_DIR=/tmp/run-output run --serve-mcp

# Or configure in Claude Desktop MCP settings
{
  "mcpServers": {
    "my-project": {
      "command": "run",
      "args": ["--serve-mcp"],
      "env": {
        "PWD": "/path/to/your/project/",
        "RUN_MCP_OUTPUT_DIR": "/path/to/output"
      }
    }
  }
}
```

**Why this matters:** When running in MCP mode, outputs longer than 32 lines are automatically truncated in the response to the AI agent, with the full output saved to a file. This prevents overwhelming the AI with massive outputs while still preserving access to the complete data.

---

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

When running in MCP mode, outputs longer than 32 lines are truncated and the full text is saved under a `.run-output` directory next to your Runfile. The directory is created automatically. To override the location (e.g., for sandboxing), set `RUN_MCP_OUTPUT_DIR` before starting the server:

```bash
RUN_MCP_OUTPUT_DIR=/tmp/run-output run --serve-mcp
```

---

## Real-World Examples

Here are practical examples demonstrating how to use `run` for real-world workflows.

### Docker Management

Organize Docker commands with nested namespaces:

```bash
# @desc Build the Docker image
docker:build() {
    docker build -t myapp:latest .
}

# @desc Start all services in detached mode
docker:up() {
    docker compose up -d
}

# @desc View logs for a service
# @arg service The service name (defaults to "app")
docker:logs(service = "app") {
    docker compose logs -f $service
}

# @desc Open a shell in a container
# @arg service The service name (defaults to "app")
docker:shell(service = "app") {
    docker compose exec $service bash
}
```

Run them like this:
```bash
$ run docker build
$ run docker up
$ run docker logs postgres
$ run docker shell
```

### CI/CD Pipeline with Function Composition

Build reusable tasks that compose together:

```bash
# @desc Run linting checks
lint() {
    cargo clippy -- -D warnings
}

# @desc Run all tests
test() {
    cargo test
}

# @desc Build release binary
build() {
    cargo build --release
}

# @desc Run the complete CI pipeline
ci() {
    echo "Running CI pipeline..."
    lint || exit 1
    test || exit 1
    build
    echo "✓ CI passed!"
}

# @desc Deploy to an environment
# @arg env Target environment (staging|prod)
deploy(env: str) {
    # Ensure build succeeds first
    build || exit 1
    echo "Deploying to $env..."
    scp target/release/myapp server-$env:/usr/local/bin/
}
```

### Polyglot Scripts - Python Data Analysis

Embed Python directly in your Runfile for data processing:

```bash
# @desc Analyze a JSON file and print statistics
# @arg file Path to the JSON file
analyze(file: str) {
    #!/usr/bin/env python3
    import sys, json
    from collections import Counter

    with open(sys.argv[1]) as f:
        data = json.load(f)

    print(f"Total records: {len(data)}")

    if isinstance(data, list) and len(data) > 0:
        keys = data[0].keys() if isinstance(data[0], dict) else []
        print(f"Fields: {', '.join(keys)}")
}

# @desc Convert CSV to JSON
# @arg input Input CSV file
# @arg output Output JSON file
csv_to_json(input: str, output: str) {
    #!/usr/bin/env python3
    import sys, csv, json

    with open(sys.argv[1]) as f:
        reader = csv.DictReader(f)
        data = list(reader)

    with open(sys.argv[2], 'w') as f:
        json.dump(data, f, indent=2)

    print(f"✓ Converted {len(data)} rows to {sys.argv[2]}")
}
```

### Secure Database Access for AI Agents

**Important security feature:** AI agents only see function descriptions and parameters via MCP—never the implementation details. This means secrets in your function bodies remain hidden.

```bash
# @desc Run a read-only query against the database
# @arg query The SQL query to execute
# @arg env The target environment (local|staging|prod)
db:query(query: str, env: str) {
    #!/usr/bin/env python3
    import sys, os, psycopg2

    query = sys.argv[1]
    target_env = sys.argv[2]

    # AI agents can't see this implementation!
    # Connection strings can be hardcoded or loaded from environment
    db_url = os.getenv(f"DB_CONNECTION_{target_env.upper()}")
    # Or: db_url = "postgresql://user:pass@localhost/mydb"

    if not db_url:
        print(f"Error: Connection not configured for {target_env}.")
        sys.exit(1)

    print(f"Executing query on {target_env}...")
    conn = psycopg2.connect(db_url)
    cur = conn.cursor()
    cur.execute(query)

    for row in cur.fetchall():
        print(row)

    cur.close()
    conn.close()
}

# @desc Get table schema information
# @arg table Table name to inspect
# @arg env Target environment
db:schema(table: str, env: str) {
    #!/usr/bin/env python3
    import sys, os, psycopg2

    table = sys.argv[1]
    target_env = sys.argv[2]

    db_url = os.getenv(f"DB_CONNECTION_{target_env.upper()}")
    conn = psycopg2.connect(db_url)
    cur = conn.cursor()

    # Query information_schema for column info
    cur.execute("""
        SELECT column_name, data_type, is_nullable
        FROM information_schema.columns
        WHERE table_name = %s
        ORDER BY ordinal_position
    """, (table,))

    print(f"\nSchema for table '{table}':")
    for col_name, data_type, nullable in cur.fetchall():
        null_str = "NULL" if nullable == "YES" else "NOT NULL"
        print(f"  {col_name}: {data_type} {null_str}")

    cur.close()
    conn.close()
}
```

**Security Benefit:** When you run `run --inspect`, the AI sees:
```json
{
  "name": "db:query",
  "description": "Run a read-only query against the database",
  "inputSchema": {
    "properties": {
      "query": {"type": "string", "description": "The SQL query to execute"},
      "env": {"type": "string", "description": "The target environment (local|staging|prod)"}
    }
  }
}
```

The function body—including any credentials, API keys, or implementation logic—is never exposed to the AI agent. This allows you to safely hardcode secrets or reference them from your environment without risk of leakage.

### Platform-Specific Commands

Define different implementations for different operating systems:

```bash
# @desc Clean build artifacts
# @os windows
clean() {
    del /Q /S target\*
    echo "Cleaned!"
}

# @desc Clean build artifacts
# @os unix
clean() {
    rm -rf target/
    echo "Cleaned!"
}

# Or use inline platform guards:
# @desc Open the project in the default editor
open() {
    @macos { open . }
    @linux { xdg-open . }
    @windows { start . }
}
```

### Node.js Web Server

Embed a Node.js server directly in your Runfile:

```bash
# @desc Start a development web server
# @arg port Port number to listen on
dev:server(port = "3000") {
    #!/usr/bin/env node
    const http = require('http');
    const fs = require('fs');
    const path = require('path');

    const port = process.argv[2] || 3000;

    http.createServer((req, res) => {
        console.log(`${req.method} ${req.url}`);

        let filePath = '.' + req.url;
        if (filePath === './') filePath = './index.html';

        fs.readFile(filePath, (err, data) => {
            if (err) {
                res.writeHead(404);
                res.end('Not found');
                return;
            }
            res.writeHead(200);
            res.end(data);
        });
    }).listen(port);

    console.log(`Server running at http://localhost:${port}/`);
}
```

---

## License

MIT