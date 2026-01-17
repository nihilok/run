# Run Codebase Development Guidelines

## Core Principles

**DRY (Don't Repeat Yourself)**
- Extract common logic into reusable functions or traits
- Use macros sparingly and only when they significantly reduce boilerplate
- Create utility modules for shared functionality across features

**Single Responsibility**
- Each function should do one thing well
- Modules should have clear, focused purposes
- Separate concerns: parsing, execution, I/O, business logic

**Composition Over Inheritance**
- Prefer trait composition and struct embedding
- Use trait objects (`dyn Trait`) or generics for polymorphism
- Build complex behavior from simple, composable pieces

## Code Organization

### Module Structure
```
src/
├── lib.rs          # Public API and module declarations
├── main.rs         # CLI entry point only
├── ast.rs          # Data structures (types, enums)
├── parser.rs       # Parsing logic
├── interpreter.rs  # Execution logic
├── utils.rs        # Shared utilities
└── feature/        # Feature-specific modules
    ├── mod.rs
    └── submodule.rs
```

**Rules:**
- Keep modules under 500 lines; split if larger
- Group related types in the same module
- Make only what's necessary `pub`
- Use `mod.rs` for module organization, not implementation

### Function Design
- **Max 50 lines per function** - extract helpers if longer
- **Max 4 parameters** - use structs for more
- **Return `Result<T, E>`** for fallible operations, not panics
- **Name functions verbally**: `parse_statement`, `execute_command`, `build_preamble`

## Error Handling

```rust
// Good: Propagate errors with context
fn load_config() -> Result<String, Box<dyn std::error::Error>> {
    config::load_config()
        .ok_or_else(|| "No Runfile found".into())
}

// Bad: Unwrap or panic
fn load_config() -> String {
    config::load_config().unwrap() // ❌
}
```

**Guidelines:**
- Use `?` operator for error propagation
- Create custom error types for domain errors
- Add context with `.map_err()` or `.ok_or_else()`
- Never `unwrap()` or `expect()` in library code

## Testing Strategy

### Test Organization
```
src/
├── module.rs
└── tests/          # Integration tests only
    └── module_test.rs

// Unit tests in same file
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_specific_behavior() {
        // Arrange
        let input = "test";
        
        // Act
        let result = parse(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}
```

### Testing Best Practices

**Unit Tests:**
- Test one behavior per test function
- Use descriptive names: `test_parse_shebang_with_python3`
- Keep tests fast (<10ms per test)
- Mock external dependencies (filesystem, network)

**Integration Tests:**
- Test end-to-end workflows
- Use `tempfile` for filesystem isolation
- Verify CLI behavior with `Command::new(binary)`
- Check both success and failure cases

**Test Coverage:**
- Every public function needs tests
- Test edge cases: empty input, invalid data, boundary values
- Test error paths, not just happy paths
- Aim for >80% coverage on critical paths

**Fixtures and Helpers:**
```rust
fn create_test_runfile(dir: &Path, content: &str) {
    fs::write(dir.join("Runfile"), content).unwrap();
}

fn get_binary_path() -> PathBuf {
    // Reusable helper for integration tests
}
```

## Code Quality Checks

### Pre-Commit Checklist
```bash
# Format
cargo fmt

# Lint
cargo clippy -- -D warnings

# Test
cargo test

# Build
cargo build --release
```

### Clippy Configuration
Enable strict lints in `Cargo.toml`:
```toml
[lints.clippy]
pedantic = "warn"
unwrap_used = "deny"
expect_used = "deny"
```

## Performance Considerations

- **Avoid cloning** - use references (`&T`) or `Cow<T>`
- **Preallocate collections** - `Vec::with_capacity()`, `HashMap::with_capacity()`
- **Use `&str` over `String`** for function parameters
- **Profile before optimizing** - use `cargo flamegraph`

## Documentation

```rust
/// Parse a script from source code.
///
/// # Arguments
/// * `input` - The script source code
///
/// # Returns
/// A parsed `Program` or parse error
///
/// # Errors
/// Returns `Err` if the input contains syntax errors
pub fn parse_script(input: &str) -> Result<Program, Box<pest::error::Error<Rule>>> {
    // ...
}
```

**Rules:**
- Document all `pub` items
- Include examples for complex functions
- Explain invariants and edge cases
- Keep docs up-to-date with code changes

## Version Control

**Commit Messages:**
```
Add shebang detection for Python scripts

- Parse #!/usr/bin/env python from function bodies
- Strip shebang before passing to interpreter
- Add tests for shebang precedence over @shell

Fixes #123
```

**Branch Strategy:**
- `main` - stable, releasable
- `feature/*` - new features
- `fix/*` - bug fixes
- `rfc/*` - experimental implementations

## Dependencies

**Adding Dependencies:**
1. Check if stdlib can solve it
2. Verify crate is maintained (recent commits)
3. Minimize dependency count
4. Pin major versions in `Cargo.toml`

**Review Dependency Tree:**
```bash
cargo tree
cargo tree --duplicate  # Find version conflicts
```

## Performance Testing

```rust
#[test]
fn bench_parse_large_runfile() {
    let large_input = generate_runfile(1000); // 1000 functions
    let start = std::time::Instant::now();
    
    let _ = parse_script(&large_input).unwrap();
    
    assert!(start.elapsed() < Duration::from_millis(100));
}
```

## Refactoring Checklist

Before refactoring:
1. ✅ All tests pass
2. ✅ Write tests for current behavior
3. ✅ One refactor at a time (no feature additions)
4. ✅ Tests still pass after refactor
5. ✅ Commit immediately if successful

## Common Patterns

### Builder Pattern
```rust
struct CommandBuilder {
    shell: String,
    args: Vec<String>,
}

impl CommandBuilder {
    fn new() -> Self { /* ... */ }
    fn shell(mut self, shell: String) -> Self {
        self.shell = shell;
        self
    }
    fn build(self) -> Command { /* ... */ }
}
```

### Error Context
```rust
fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))
}
```

### Lazy Initialization
```rust
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| Config::load())
}
```

## Anti-Patterns to Avoid

❌ **Stringly-typed APIs** - use enums
❌ **Deep nesting** - extract functions  
❌ **Global mutable state** - use parameters
❌ **Boolean parameters** - use enums for clarity
❌ **Large match arms** - delegate to functions
❌ **Ignoring errors** - always handle or propagate

## Maintenance

**Regular Tasks:**
- Update dependencies monthly: `cargo update`
- Run `cargo audit` for security issues
- Review and close stale issues
- Update documentation for new features
- Add regression tests for bugs

**Release Process:**
1. Update `CHANGELOG.md`
2. Bump version in `Cargo.toml`
3. Run full test suite
4. Tag release: `git tag v0.x.y`
5. Verify CI passes before publish