# RFC 001: Interpreter Attributes & Platform Guarding

## Metadata

| Detail             | Value                                             |
|--------------------|---------------------------------------------------|
| **Status**         | Draft                                             |
| **Target Version** | v0.2.1                                            |
| **Type**           | Feature                                           |
| **Topic**          | Cross-platform compatibility & Polyglot scripting |

---

## 1. Summary

This proposal introduces **Attribute Comments** (Magic Comments) to the Runfile syntax. These attributes allow
developers to:

- **Guard functions by OS** (e.g., distinct Windows vs. Linux implementations)
- **Select the interpreter** for a specific function (e.g., executing a block via Python, Node, or PowerShell instead of
  the default shell)

---

## 2. Motivation

Currently, `run` executes all functions using the user's default shell (usually `sh` on Unix, `cmd` or `pwsh` on
Windows). This creates two friction points:

1. **Incompatibility**: A Runfile created on macOS often fails on Windows due to missing binaries (`ls`, `rm`, `grep`)
2. **Limited Logic**: Shell scripting is inefficient for complex tasks (math, JSON parsing, API calls), forcing users to
   create external script files and breaking the "single file" value proposition of `run`

---

## 3. Syntax Specification

Attributes are defined using comments starting with `# @` immediately preceding the function definition. This approach
preserves syntax highlighting in most editors, unlike custom macro syntax.

### 3.1. Platform Guards (`@os`)

Restricts the availability or execution of a function based on the operating system.

**Syntax**: `# @os <windows|linux|macos|unix>`

```bash
# @os windows
clean() {
    del /Q dist
}

# @os unix
clean() {
    rm -rf dist
}
```

- If a user runs `run clean` on Linux, the Windows variant is ignored, and the Unix variant is executed
- If no matching variant is found for the current OS, `run` should exit with a helpful error

### 3.2. Interpreter Selection (`@shell` / `@lang`)

Defines the binary used to execute the function body.

**Syntax**: `# @shell <interpreter>` or `# @lang <interpreter>`

**Supported Interpreters** (Initial List):

- `python` / `python3`
- `node`
- `ruby`
- `pwsh` / `powershell`
- `bash` / `zsh` / `sh`

```python
# @shell python
math() {
    import sys
    print(10 + 20)
}
```

```javascript
# @shell
node
server()
{
    console.log("Starting server...");
}
```

---

## 4. Technical Implementation

### 4.1. Parser Logic

The parser currently scans for `name() {`. It must be updated to peek at the preceding lines.

- **Scan Upwards**: Upon finding a function, scan upward for lines starting with `# @`
- **Stop Condition**: Stop scanning if a blank line or a non-attribute line is found
- **Association**: Store the attributes in the `Function` struct

```rust
struct Function {
    name: String,
    body: String,
    // New fields
    required_os: Option<OsType>,
    interpreter: Option<String>,
}
```

### 4.2. Execution Strategy (The Polyglot Bridge)

When executing a function with a custom `@shell`:

1. **Construct Command**: Do not use the default shell. Instead, spawn the specific interpreter
2. **Pass Body**: Pass the function body string to the interpreter's "command" flag (usually `-c` or `-e`)
3. **Pass Arguments**: Forward the user's arguments (`run task arg1 arg2`) to the interpreter

**Interpreter Mapping Table**:

| Identifier           | Binary   | Command Flag | Argument Handling                        |
|----------------------|----------|--------------|------------------------------------------|
| `python`, `python3`  | `python` | `-c`         | `python -c "<body>" arg1 arg2`           |
| `node`               | `node`   | `-e`         | `node -e "<body>" arg1 arg2`             |
| `ruby`               | `ruby`   | `-e`         | `ruby -e "<body>" arg1 arg2`             |
| `pwsh`, `powershell` | `pwsh`   | `-Command`   | `pwsh -Command "<body>" -args arg1 arg2` |
| `bash`, `sh`         | `bash`   | `-c`         | `bash -c "<body>" $0 arg1 arg2`          |

### 4.3. Argument Accessibility (Stdio)

We must ensure the user's arguments are accessible inside the script language.

**Python Example**:

- User runs: `run calc 50`
- Generated Command: `python -c "import sys; print(sys.argv[1])" 50`

**Node Example**:

- User runs: `run log hello`
- Generated Command: `node -e "console.log(process.argv[1])" hello`

---

## 5. Edge Cases

### 5.1. Quoting

Because the body is passed as a string argument to the interpreter, we rely on Rust's `std::process::Command` to handle
escaping.

**Note**: The user does not need to escape quotes inside their script body.

### 5.2. Imports

Code inside `{}` is ephemeral.

- Standard library imports work immediately
- Third-party imports (e.g., `import requests`) only work if the package is installed in the system environment or a
  local `venv`/`node_modules`

### 5.3. Editor Compatibility

We chose the `# @` syntax over `[#macro]` syntax because editors (VS Code, Vim, Sublime) treat `#` as a comment. This
ensures syntax highlighting for the file remains valid (mostly), preventing the file from looking broken in IDEs.

---

## 6. Future Considerations (v2)

- **Shebang Detection**: If the first line of the function body is `#!/usr/bin/env python`, auto-detect the
  interpreter (similar to `just`)
- **Dependency Chaining**: An attribute like `# @depends clean build` to run prerequisites
- **Working Directory**: `# @cd ./backend` to run the function in a subfolder