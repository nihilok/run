# DRY Refactoring: Extracting Shared Logic

**Project:** RFC006 Structured Output Implementation  
**Date:** January 19, 2026  
**Context:** Eliminating code duplication during feature implementation

---

## The Problem

During RFC006 implementation, we found duplicated logic:

1. **Shell command mapping** duplicated in two places
2. **Output format conversion** split across multiple functions  
3. **Interpreter name** hardcoded instead of tracked

## Refactoring 1: Extract Helper Functions

### Before: Duplicated Mapping

```rust
// In interpreter/mod.rs
let (shell_cmd, shell_arg) = match interpreter {
    TranspilerInterpreter::Sh => ("sh", "-c"),
    TranspilerInterpreter::Bash => ("bash", "-c"),
    TranspilerInterpreter::Python => (get_python_executable(), "-c"),
    // ... repeated pattern
};

// Same pattern in shell.rs - violation of DRY!
```

### After: Shared Helper

```rust
// In shell.rs - single source of truth
pub(super) fn interpreter_to_shell_args(
    interpreter: &TranspilerInterpreter
) -> (String, &'static str, &'static str) {
    match interpreter {
        TranspilerInterpreter::Sh => ("sh".to_string(), "-c", "sh"),
        TranspilerInterpreter::Bash => ("bash".to_string(), "-c", "bash"),
        TranspilerInterpreter::Python => (get_python_executable(), "-c", "python"),
        // ...
    }
}

// Usage - both callers use the same function
let (shell_cmd, shell_arg, interpreter_name) = interpreter_to_shell_args(interpreter);
```

**Bonus:** Returns interpreter name too, eliminating hardcoded `"sh"`.

## Refactoring 2: Consolidate Related Parameters

### Before: Multiple Parameters

```rust
pub fn run_function_call(
    function_name: &str,
    args: &[String],
    output_mode: OutputMode,        // Converted from format
    format_as_markdown: bool,       // Derived from same source!
) { ... }

// Caller has redundant logic
let output_mode = cli.output_format.into();
let format_as_markdown = matches!(cli.output_format, OutputFormatArg::Markdown);
```

### After: Single Source with Methods

```rust
pub enum OutputFormatArg {
    Stream,
    Json,
    Markdown,
}

impl OutputFormatArg {
    pub fn mode(self) -> OutputMode { ... }
    pub fn format_result(self, result: &StructuredResult) -> Option<String> { ... }
}

// Caller is simple
pub fn run_function_call(
    function_name: &str,
    args: &[String],
    output_format: OutputFormatArg,  // Single parameter
) { ... }
```

## Refactoring 3: Track State Instead of Hardcoding

### Before: Hardcoded Value

```rust
let interpreter_name = "sh"; // TODO: track actual interpreter
let result = StructuredResult::from_outputs(name, outputs, interpreter_name);
```

### After: Track and Use

```rust
// Add field to track state
pub struct Interpreter {
    // ... other fields
    last_interpreter: String,
}

// Update when executing
fn execute_with_mode(&mut self, ...) {
    let (_, _, interpreter_name) = interpreter_to_shell_args(interpreter);
    self.last_interpreter = interpreter_name.to_string();
    // ...
}

// Use tracked value
let result = StructuredResult::from_outputs(
    name, 
    outputs, 
    interpreter.last_interpreter()
);
```

## When to Refactor

Refactor when you see:

1. **Copy-paste code** - Same match arms, same conditionals
2. **Derived values passed separately** - Multiple params from same source
3. **Hardcoded placeholders** - `"TODO"` or `"sh"` where real data exists
4. **Dead code warnings** - Unused functions often indicate duplication

## Checklist

- [ ] Look for duplicate match statements
- [ ] Check if multiple function params derive from same source
- [ ] Search for hardcoded strings that could be computed
- [ ] Run `cargo clippy` and address all warnings
- [ ] Verify tests still pass after refactoring

## Result

After these refactorings:
- Zero code duplication for shell mapping
- Single `OutputFormatArg` parameter instead of two
- Actual interpreter name tracked and used
- No compiler warnings about dead code
