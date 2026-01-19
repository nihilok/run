# Backward-Compatible Feature Addition Pattern

**Project:** RFC006 Structured Output Implementation  
**Date:** January 19, 2026  
**Context:** Adding `--output-format` flag without breaking existing workflows

---

## The Goal

Add new functionality (structured output capture) while ensuring:
1. Existing users experience zero changes by default
2. New functionality is opt-in
3. All existing tests continue to pass

## The Pattern: Default-Preserving Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OutputMode {
    #[default]
    Stream,      // ← Existing behavior (default)
    Capture,     // ← New: capture + stream  
    Structured,  // ← New: capture only, format at end
}
```

**Key:** Use `#[default]` to make the existing behavior the default.

## CLI Integration

Use clap's `default_value` to preserve backward compatibility:

```rust
#[arg(long, value_name = "FORMAT", default_value = "stream")]
output_format: OutputFormatArg,
```

Users who don't specify `--output-format` get the old behavior.

## Implementation Strategy

### 1. Check Mode at Decision Points

```rust
match self.output_mode {
    OutputMode::Stream => {
        // Original code path - unchanged behavior
        shell::execute_single_shell_invocation(script, interpreter)
    }
    OutputMode::Capture | OutputMode::Structured => {
        // New code path - capture output
        self.execute_with_capture(script, interpreter)
    }
}
```

### 2. Format Output Only When Requested

```rust
// Only format when user requested structured output
if matches!(output_format.mode(), OutputMode::Structured) {
    let outputs = interpreter.take_captured_outputs();
    if !outputs.is_empty() {
        if let Some(formatted) = output_format.format_result(&result) {
            println!("{}", formatted);
        }
    }
}
// Otherwise: output was already streamed to terminal
```

### 3. Test Both Paths

```rust
#[test]
fn test_default_stream_mode_no_capture() {
    // Verify default behavior hasn't changed
    let output = Command::new(binary)
        .arg("test_func")  // No --output-format flag
        .output()?;
    
    // Should NOT contain JSON or markdown
    assert!(!stdout.contains("\"function_name\""));
    assert!(!stdout.contains("## Execution:"));
    // Should contain raw output
    assert!(stdout.contains("Direct output"));
}
```

## Checklist for Backward-Compatible Features

- [ ] Default enum variant preserves existing behavior
- [ ] CLI flag has `default_value` matching old behavior
- [ ] Mode check at every decision point
- [ ] New functionality only activates when explicitly requested
- [ ] Integration test verifying default behavior unchanged
- [ ] All existing tests pass without modification

## Anti-Patterns to Avoid

### ❌ Changing Default Behavior
```rust
// Don't do this - breaks existing users
pub enum OutputMode {
    #[default]
    Structured,  // ← New behavior as default!
    Stream,
}
```

### ❌ Unconditional New Behavior
```rust
// Don't do this - affects all users
fn execute(&mut self, ...) {
    self.capture_output();  // ← Always captures now
    // ...
}
```

### ❌ Forgetting Backward Compat Tests
```rust
// Always test that the OLD way still works
#[test]
fn test_existing_behavior_unchanged() { ... }
```

## Result

With this pattern:
- 100% of existing Runfiles work unchanged
- New `--output-format` flag is fully opt-in
- All 239 existing tests pass without modification
- New tests validate the new functionality
