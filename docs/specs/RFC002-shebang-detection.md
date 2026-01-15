# RFC 002: Shebang Detection

## Metadata

| Detail             | Value                                  |
|--------------------|----------------------------------------|
| **Status**         | Completed                              |
| **Target Version** | v0.2.2                                 |
| **Type**           | Feature                                |
| **Topic**          | Shebang detection within function body |

## Summary

Add automatic interpreter detection via shebang lines (`#!/usr/bin/env python`) as an alternative to explicit `@shell` attributes. This provides a more familiar syntax for users coming from traditional shell scripting.

## Motivation

Currently, users must explicitly declare interpreters using attributes:

```python
# @shell python
analyze() {
    import sys
    print("Hello")
}
```

Many developers are accustomed to shebangs from standalone scripts and tools like `just`. Supporting shebang detection:

1. **Reduces verbosity** - No need for both shebang and `@shell` attribute
2. **Familiar syntax** - Leverages existing shell scripting conventions
3. **Self-documenting** - The interpreter is visible inside the function body
4. **Tool compatibility** - Code can be extracted to standalone scripts more easily

## Syntax

If the **first non-empty line** of a function body is a shebang, use it to determine the interpreter.

```python
analyze() {
    #!/usr/bin/env python
    import sys
    print(f"Analyzing {sys.argv[1]}")
}
```

```javascript
server() {
    #!/usr/bin/env node
    const port = process.argv[1] || 3000;
    console.log(`Server on port ${port}`);
}
```

```bash
setup() {
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Setting up..."
}
```

## Precedence Rules

When both shebang and `@shell` attribute are present, **`@shell` takes precedence**:

```python
# @shell python3
calc() {
    #!/usr/bin/env python
    # Will execute with python3, not python
    print("Hello")
}
```

This allows users to override the shebang for testing or compatibility purposes.

## Implementation

### 4.1. Parser Changes

When parsing a function body:

1. Extract the first non-empty, non-comment line (ignoring `#` lines that aren't shebangs)
2. Check if it matches the pattern: `^#!/.+$`
3. If matched, extract the interpreter path

```rust
struct Function {
    name: String,
    body: String,
    required_os: Option<OsType>,
    interpreter: Option<String>,
    shebang: Option<String>, // New field
}

fn parse_shebang(body: &str) -> Option<String> {
    body.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .next()
        .and_then(|line| {
            if line.starts_with("#!") {
                Some(line[2..].trim().to_string())
            } else {
                None
            }
        })
}
```

### 4.2. Interpreter Resolution

The shebang path must be resolved to a binary name. Common patterns:

| Shebang | Resolved Binary | Command Flag |
|---------|----------------|--------------|
| `#!/usr/bin/env python` | `python` | `-c` |
| `#!/usr/bin/env python3` | `python3` | `-c` |
| `#!/usr/bin/env node` | `node` | `-e` |
| `#!/usr/bin/env ruby` | `ruby` | `-e` |
| `#!/usr/bin/env bash` | `bash` | `-c` |
| `#!/bin/bash` | `bash` | `-c` |
| `#!/bin/sh` | `sh` | `-c` |

Resolution strategy:

1. If shebang contains `/env `, extract everything after it (e.g., `python3`)
2. Otherwise, extract the basename (e.g., `/bin/bash` → `bash`)
3. Look up the binary in the interpreter mapping table (from RFC 001)

```rust
fn resolve_interpreter(shebang: &str) -> Option<String> {
    if let Some(env_part) = shebang.strip_prefix("/usr/bin/env ") {
        Some(env_part.split_whitespace().next()?.to_string())
    } else {
        std::path::Path::new(shebang)
            .file_name()?
            .to_str()
            .map(String::from)
    }
}
```

### 4.3. Execution Strategy

When executing a function:

1. **Check for explicit `@shell` attribute** → use it if present
2. **Check for shebang** → resolve and use interpreter
3. **Fall back to default shell** → use `RUN_SHELL` or system default

The shebang line should be **stripped from the body** before passing to the interpreter to avoid syntax errors:

```rust
fn strip_shebang(body: &str) -> String {
    body.lines()
        .skip_while(|l| l.trim().starts_with("#!"))
        .collect::<Vec<_>>()
        .join("\n")
}
```

Example execution:

```python
analyze() {
    #!/usr/bin/env python
    import sys
    print(sys.argv[1])
}
```

User runs: `run analyze data.json`

Generated command: `python -c "import sys\nprint(sys.argv[1])" data.json`

(Note: shebang line is removed)

## Edge Cases

### 5.1. Shebang with Arguments

Some shebangs include flags: `#!/usr/bin/env python -u`

**Decision**: Extract only the binary name (`python`), ignore flags. The function body can set flags via code (e.g., `sys.stdout = os.fdopen(sys.stdout.fileno(), 'w', 0)` for unbuffered output).

**Rationale**: Passing arbitrary flags through the `-c` interface is unreliable and interpreter-specific.

### 5.2. Multi-line Shebangs

Only the **first line** is checked. Multi-line shebangs are not supported:

```python
# Invalid - will not detect
analyze() {
    #!/usr/bin/env \
      python
}
```

### 5.3. Shebang in Middle of Body

Only the **first non-empty line** is checked. Shebangs elsewhere are ignored:

```python
# Will execute with default shell, not Python
broken() {
    echo "Hello"
    #!/usr/bin/env python
}
```

### 5.4. Unknown Interpreters

If the resolved interpreter is not in the mapping table, emit a warning and fall back to the default shell:

```
Warning: Unknown interpreter 'perl' in shebang. Falling back to default shell.
```

### 5.5. Comments Before Shebang

Lines starting with `#` but not `#!` are ignored when searching for shebangs:

```python
analyze() {
    # This is a comment
    #!/usr/bin/env python  # ← This is detected
    import sys
}
```

## User Experience

### Example: Polyglot Runfile

```bash
# Runfile

# Traditional shell
build() cargo build --release

# Shebang detection
test() {
    #!/usr/bin/env bash
    cargo test --all
    echo "Tests passed!"
}

# Python via shebang
stats() {
    #!/usr/bin/env python3
    import sys, json
    with open(sys.argv[1]) as f:
        print(len(json.load(f)), "records")
}

# Node via attribute (explicit override)
# @shell node
server() {
    #!/usr/bin/env python  # Ignored - @shell takes precedence
    console.log("Server starting...");
}
```

### Migration Path

Users can gradually adopt shebangs:

**Before** (explicit attributes):
```python
# @shell python
calc() {
    import math
    print(math.pi)
}
```

**After** (shebang):
```python
calc() {
    #!/usr/bin/env python
    import math
    print(math.pi)
}
```

Both styles remain supported indefinitely.

## Compatibility

- **Backward compatible**: Existing Runfiles without shebangs continue to work
- **Attribute precedence**: `@shell` overrides shebangs, allowing explicit control
- **Graceful degradation**: Unknown interpreters fall back to default shell with a warning

## Future Considerations

- **Shebang arguments**: Support flags like `#!/usr/bin/env python -u` (complex, needs per-interpreter handling)
- **Custom interpreter paths**: Allow absolute paths like `#!/opt/homebrew/bin/python3` (requires PATH resolution)
- **Validation**: Warn if shebang interpreter is not installed on the system