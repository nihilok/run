# RFC 005: Internal Function Composition (Scope Injection)

**Status**: Draft | **Type**: Feature | **Target**: v0.3.1  
**Topic**: Code Reuse & Task Dependency

## 1. Summary

This proposal enables Runfile functions to call other functions defined in the same file directly, without spawning a new `run` process or relying on external scripts.

This is achieved by **Context Injection**: When `run` executes a target function, it automatically pre-loads all other "compatible" functions into the execution scope.

---

## 2. Motivation

Currently, functions in a Runfile are isolated. If a user defines `build` and `deploy`, they cannot easily compose them into a `ci` task.

**Current Workarounds (Bad):**
- **Duplication**: Copy-pasting the `build` logic into `deploy`.
- **Recursion**: Calling `run build` inside `deploy`. This is slow (spawns new process, reparses file) and relies on `run` being in the system `$PATH`.
- **External Scripts**: Moving logic to `./scripts/build.sh`, defeating the purpose of a task runner.

**Desired Behavior:**

```bash
build() cargo build --release
test() cargo test

# Native composition
ci() {
    build  # Calls the sibling function directly
    test
}
```

---

## 3. Design Considerations

### 3.1. Simple vs Block Functions

The current interpreter distinguishes between:
- **Simple functions**: `build() cargo build` - stored as a command template string
- **Block functions**: `build() { cargo build; cargo test }` - stored as a list of commands

For scope injection, both must be transpiled to proper shell function syntax.

### 3.2. Current Execution Model

Currently in `execute_block_commands()`:
- For shell functions (no `@shell` attribute): commands are executed **one by one** via separate `sh -c` calls
- For custom shells (Python, Node): commands are joined and passed as a single script

This per-command execution model means sibling functions are **not in scope**. The fix requires building a single shell script payload with all compatible functions.

### 3.3. Function Name Colon Syntax

Runfile supports `docker:shell` notation which is invalid as a shell function name. These must be transpiled:
- `docker:shell` → `docker__shell` (double underscore)
- Call sites must also be rewritten

### 3.4. Return Value / Exit Code Handling

When a composed function calls a sibling, the exit code matters:

```bash
ci() {
    build || exit 1   # Stop if build fails
    test
}
```

The injected preamble must preserve exit code semantics. Each injected function should propagate its exit status naturally.

### 3.5. Variables Defined at Runfile Top-Level

Currently, top-level assignments like `VERSION="1.0.0"` are stored in `interpreter.variables` but **not passed to shell execution**. This RFC should also inject these as shell variables in the preamble.

---

## 4. Technical Implementation

### 4.1. The "Preamble" Strategy

When `run` prepares to execute a target function (e.g., `ci`), it constructs a combined script:

1. **Variable Preamble**: Export all top-level Runfile variables
2. **Function Preamble**: Define all compatible sibling functions
3. **Target Body**: The body of the target function

**Logical Flow:**

1. User runs `run ci`.
2. `run` parses the Runfile, loading all functions and variables.
3. `run` identifies the interpreter for `ci` (e.g., `sh`, `bash`, or `@shell pwsh`).
4. `run` filters sibling functions by:
   - Same interpreter compatibility (see 4.2)
   - Same `@os` platform requirements
5. `run` transpiles each sibling to shell function syntax.
6. `run` builds: `[variables] + [function defs] + [target body]`
7. `run` executes the combined payload via a single shell invocation.

### 4.2. Scope Matching Rules

We can only inject functions that share compatible syntax.

| Target Interpreter | Compatible Siblings | Notes |
|-------------------|---------------------|-------|
| `sh` | `sh`, `bash` (without bash-isms) | POSIX function syntax `name() { ... }` |
| `bash` | `sh`, `bash` | Bash is superset of POSIX sh |
| `pwsh`, `powershell` | `pwsh`, `powershell` | PowerShell function syntax `function name { ... }` |
| `python`, `python3` | None (self only) | Cannot import sibling blocks without AST work |
| `node` | None (self only) | Same as Python |
| `ruby` | None (self only) | Same as Python |

**Default (no `@shell`)**: Treated as `sh` on Unix, `pwsh` on Windows.

### 4.3. Transpilation Rules

#### Shell (sh/bash)

**Simple function** `build() cargo build --release`:
```bash
build() {
    cargo build --release
}
```

**Block function**:
```bash
build() {
    cargo build --release
    cargo test
}
```

**Colon names** `docker:shell()` → `docker__shell()`:
```bash
docker__shell() {
    docker compose exec ${1:-app} bash
}
```

#### PowerShell (pwsh)

**Simple function** `build() cargo build`:
```powershell
function build {
    cargo build
}
```

**Block function**:
```powershell
function build {
    cargo build
    cargo test
}
```

**Colon names** `docker:shell` → `docker__shell`:
```powershell
function docker__shell {
    docker compose exec $args[0] bash
}
```

### 4.4. Call Site Rewriting

When the target function body contains calls to colon-named siblings, those must be rewritten:

**Original:**
```bash
ci() {
    docker:build
    docker:push
}
```

**Transpiled:**
```bash
ci() {
    docker__build
    docker__push
}
```

**Implementation**: Before emitting the target body, perform string replacement:
- For each sibling with `:` in name, replace `siblingname` → `sibling__name` in the body

### 4.5. Variable Injection

Top-level Runfile assignments should be injected as shell variables:

**Runfile:**
```bash
VERSION="1.0.0"
PROJECT="myapp"

build() echo "Building $PROJECT v$VERSION"
```

**Generated Preamble:**
```bash
VERSION="1.0.0"
PROJECT="myapp"

build() {
    echo "Building $PROJECT v$VERSION"
}
```

### 4.6. Argument Passing in Composed Calls

When a sibling function is called with arguments, standard shell argument passing works:

```bash
install() echo "Installing $1..."

deploy() {
    install "package-name"   # Works naturally
    install "$1"             # Passes deploy's first arg to install
}
```

The `$1`, `$2`, etc. in `install` refer to **install's arguments**, not the outer function's.

---

## 5. Edge Cases

### 5.1. Name Collisions with System Binaries

If a Runfile function has the same name as a system binary (e.g., `git`, `docker`), the injected function takes precedence.

**Example:**
```bash
git() {
    echo "Intercepted git call: $@"
    command git "$@"   # Use 'command' to bypass function
}
```

**Behavior**: This is intentional and allows users to wrap/alias tools. Document this clearly.

### 5.2. Circular Dependencies

If `task_a` calls `task_b`, and `task_b` calls `task_a`:
- **Shell behavior**: Stack overflow, eventual crash
- **run behavior**: Does not detect or prevent this; relies on shell's natural limits
- **Recommendation**: Document as user responsibility

### 5.3. Polyglot Barriers

A `@shell python` function **cannot** call a `@shell bash` function natively.

**Workaround**: The user must use process recursion:
```python
# @shell python
deploy() {
    import subprocess
    subprocess.run(["run", "build"])  # Spawns new run process
}
```

**UX**: `run` should emit a warning if a Python/Node/Ruby function body contains what looks like a sibling function call that won't work.

### 5.4. Functions with `@os` Restrictions

A function marked `@os windows` should only be injected when running on Windows.

**Implementation**: The current `matches_current_platform()` check already filters at parse time. Scope injection should respect this—only inject functions that passed platform filtering.

### 5.5. Shebangs in Block Functions

Block functions can have shebangs (`#!/usr/bin/env python3`). These:
- Override the default interpreter
- Should be treated like `@shell python` for compatibility matching
- The shebang line must be stripped when transpiling to preamble

### 5.6. Simple Functions Calling Block Functions (and vice versa)

Both function types should be injectable and callable from each other, as long as interpreters match:

```bash
# Simple function
build() cargo build

# Block function that calls simple
ci() {
    build        # Works - both are shell
    cargo test
}
```

---

## 6. Implementation Plan

### Phase 1: Transpiler Module

Create `src/transpiler.rs`:

```rust
pub struct TranspiledFunction {
    pub name: String,           // Sanitized name (colons → __)
    pub original_name: String,  // Original name for mapping
    pub body: String,           // Full function definition
    pub interpreter: Interpreter,
}

pub enum Interpreter {
    Sh,
    Bash,
    Pwsh,
    Python,
    Node,
    Ruby,
}

impl Interpreter {
    pub fn is_compatible_with(&self, other: &Interpreter) -> bool {
        matches!(
            (self, other),
            (Interpreter::Sh, Interpreter::Sh)
            | (Interpreter::Sh, Interpreter::Bash)
            | (Interpreter::Bash, Interpreter::Sh)
            | (Interpreter::Bash, Interpreter::Bash)
            | (Interpreter::Pwsh, Interpreter::Pwsh)
        )
    }
}

/// Transpile a Runfile function to shell syntax
pub fn transpile_to_shell(name: &str, body: &str, is_block: bool) -> String {
    let sanitized = name.replace(':', "__");
    if is_block {
        format!("{}() {{\n{}\n}}", sanitized, indent(body, "    "))
    } else {
        format!("{}() {{\n    {}\n}}", sanitized, body)
    }
}

/// Transpile to PowerShell syntax
pub fn transpile_to_pwsh(name: &str, body: &str) -> String {
    let sanitized = name.replace(':', "__");
    format!("function {} {{\n{}\n}}", sanitized, indent(body, "    "))
}

/// Rewrite call sites in body (docker:build → docker__build)
pub fn rewrite_call_sites(body: &str, siblings: &[&str]) -> String {
    let mut result = body.to_string();
    for sibling in siblings {
        if sibling.contains(':') {
            let sanitized = sibling.replace(':', "__");
            // Replace whole-word occurrences
            result = regex_replace_word(&result, sibling, &sanitized);
        }
    }
    result
}
```

### Phase 2: Modify Interpreter

Update `execute_block_commands()` to build preamble:

```rust
fn execute_block_commands(
    &self,
    commands: &[String],
    args: &[String],
    attributes: &[Attribute],
    shebang: Option<&str>,
    target_name: &str,  // NEW: to exclude self from preamble
) -> Result<(), Box<dyn std::error::Error>> {
    // Determine target interpreter
    let target_interp = self.resolve_interpreter(attributes, shebang);
    
    // Build preamble of compatible siblings
    let preamble = self.build_function_preamble(target_name, &target_interp);
    let var_preamble = self.build_variable_preamble();
    
    // Rewrite call sites in body
    let body = commands.join("\n");
    let rewritten_body = transpiler::rewrite_call_sites(&body, &self.get_sibling_names());
    
    // Combine and execute
    let full_script = format!("{}\n{}\n{}", var_preamble, preamble, rewritten_body);
    let substituted = self.substitute_args(&full_script, args);
    
    self.execute_single_shell_invocation(&substituted, &target_interp)?;
    Ok(())
}

fn build_function_preamble(&self, exclude: &str, target_interp: &Interpreter) -> String {
    let mut preamble = String::new();
    
    for (name, _) in &self.simple_functions {
        if name == exclude { continue; }
        let interp = self.resolve_function_interpreter(name);
        if !target_interp.is_compatible_with(&interp) { continue; }
        
        let body = self.simple_functions.get(name).unwrap();
        preamble.push_str(&transpiler::transpile_to_shell(name, body, false));
        preamble.push_str("\n\n");
    }
    
    for (name, commands) in &self.block_functions {
        if name == exclude { continue; }
        let interp = self.resolve_function_interpreter(name);
        if !target_interp.is_compatible_with(&interp) { continue; }
        
        let body = commands.join("\n");
        preamble.push_str(&transpiler::transpile_to_shell(name, &body, true));
        preamble.push_str("\n\n");
    }
    
    preamble
}

fn build_variable_preamble(&self) -> String {
    self.variables
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join("\n")
}
```

### Phase 3: Single Shell Invocation

Currently `execute_block_commands` loops and calls shell multiple times. Change to single invocation:

```rust
fn execute_single_shell_invocation(
    &self,
    script: &str,
    interpreter: &Interpreter,
) -> Result<(), Box<dyn std::error::Error>> {
    let (shell_cmd, shell_arg) = match interpreter {
        Interpreter::Sh => ("sh", "-c"),
        Interpreter::Bash => ("bash", "-c"),
        Interpreter::Pwsh => ("pwsh", "-Command"),
        // ...
    };
    
    let status = Command::new(shell_cmd)
        .arg(shell_arg)
        .arg(script)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    
    if !status.success() {
        return Err(format!("Command failed: {}", status).into());
    }
    Ok(())
}
```

---

## 7. Example: Full Transpilation

**Runfile:**
```bash
VERSION="1.0.0"

build() cargo build --release
test() cargo test

docker:build() docker build -t myapp:$VERSION .
docker:push() docker push myapp:$VERSION

# @desc Full CI pipeline
ci() {
    build
    test
    docker:build
    docker:push
}
```

**Generated Script for `run ci`:**
```bash
# --- Variables ---
VERSION="1.0.0"

# --- Function Preamble ---
build() {
    cargo build --release
}

test() {
    cargo test
}

docker__build() {
    docker build -t myapp:$VERSION .
}

docker__push() {
    docker push myapp:$VERSION
}

# --- Target Execution ---
build
test
docker__build
docker__push
```

---

## 8. Testing Strategy

### Unit Tests

```rust
#[test]
fn test_transpile_simple_function() {
    let result = transpile_to_shell("build", "cargo build", false);
    assert_eq!(result, "build() {\n    cargo build\n}");
}

#[test]
fn test_transpile_colon_name() {
    let result = transpile_to_shell("docker:build", "docker build .", false);
    assert!(result.starts_with("docker__build()"));
}

#[test]
fn test_rewrite_call_sites() {
    let body = "docker:build\ndocker:push";
    let siblings = vec!["docker:build", "docker:push"];
    let result = rewrite_call_sites(body, &siblings);
    assert_eq!(result, "docker__build\ndocker__push");
}

#[test]
fn test_interpreter_compatibility() {
    assert!(Interpreter::Bash.is_compatible_with(&Interpreter::Sh));
    assert!(!Interpreter::Pwsh.is_compatible_with(&Interpreter::Sh));
    assert!(!Interpreter::Python.is_compatible_with(&Interpreter::Sh));
}
```

### Integration Tests

```rust
#[test]
fn test_function_composition() {
    let runfile = r#"
        build() echo "building"
        test() echo "testing"
        ci() { build; test; }
    "#;
    // Execute ci, verify both "building" and "testing" appear in output
}

#[test]
fn test_colon_function_composition() {
    let runfile = r#"
        docker:build() echo "docker build"
        ci() { docker:build; }
    "#;
    // Execute ci, verify "docker build" appears
}

#[test]
fn test_variable_injection() {
    let runfile = r#"
        VERSION="1.0.0"
        build() echo "v$VERSION"
    "#;
    // Execute build, verify "v1.0.0" appears
}
```

---

## 9. Open Questions

1. **Should we support explicit `@depends` syntax?**
   - e.g., `# @depends build, test` to declare dependencies without calling
   - *Recommendation*: Not in v1. Implicit composition via function calls is clearer.

2. **Should polyglot functions warn about incompatible sibling calls?**
   - *Recommendation*: Yes, emit warning at parse/load time if body contains sibling-like tokens.

3. **Should we cache the transpiled preamble?**
   - *Recommendation*: Not initially. Preamble generation is fast; optimize if profiling shows need.

4. **What about `set -e` (errexit) behavior?**
   - *Recommendation*: Do not inject `set -e` by default. Let users add it explicitly if desired. Document that without `set -e`, failures in composed calls won't stop execution unless explicitly handled.

---

## 10. Future Improvements (v0.5+)

### Selective Injection

Only inject functions that are **actually called** by the target, reducing preamble size.

**Implementation**: Parse target body for sibling function names, then transitively include their dependencies.

### Cross-Polyglot Composition

Enable Python functions to call shell functions via a `run.call("build")` helper that `run` injects:

```python
# @shell python
deploy() {
    run.call("build")  # Injected helper that invokes run subprocess
    print("Deploying...")
}
```

### Makefile-style Dependencies

```bash
# @depends build test
deploy() {
    ./scripts/deploy.sh
}
```

Automatically run `build` and `test` before `deploy`, with caching/skip if already run.
