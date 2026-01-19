# Adding Mutable State to Immutable Methods: Borrow Checker Patterns

**Project:** RFC006 Structured Output Implementation  
**Date:** January 19, 2026  
**Context:** Adding output capture required changing `&self` to `&mut self`

---

## The Problem

When implementing output capture for the `run` interpreter, we needed to store captured outputs in the interpreter's state. This required changing method signatures from `&self` to `&mut self`.

```rust
// Before: Immutable borrow
fn execute_block_commands(&self, ...) -> Result<...>

// After: Mutable borrow needed for capture
fn execute_block_commands(&mut self, ...) -> Result<...>
```

## The Borrow Checker Challenge

The change caused compilation errors because we were borrowing `self` immutably to read metadata, then trying to pass it mutably to a method:

```rust
// ❌ This fails
let (attributes, shebang) = self.get_block_function_metadata(function_name);
self.execute_block_commands(function_name, &commands, args, &attributes, shebang);
//   ^^^^ cannot borrow as mutable because also borrowed as immutable
```

The problem: `shebang` is a `Option<&str>` that borrows from `self.function_metadata`, but then we need `&mut self` for the method call.

## The Solution: Clone Before Mutable Borrow

Clone the borrowed data before the mutable operation:

```rust
// ✅ This works
let (attributes, shebang) = self.get_block_function_metadata(function_name);
let shebang_owned = shebang.map(String::from);  // Clone the borrowed &str
self.execute_block_commands(
    function_name, 
    &commands, 
    args, 
    &attributes, 
    shebang_owned.as_deref()  // Convert back to Option<&str>
);
```

## Key Pattern

When transitioning from `&self` to `&mut self`:

1. **Identify borrowed data** that crosses the mutation boundary
2. **Clone/own the data** before the mutable call
3. **Convert back** if the API expects references

```rust
// General pattern
let borrowed_data = self.get_something();      // Immutable borrow
let owned_data = borrowed_data.to_owned();     // Clone to break borrow
self.mutate_something(&owned_data);            // Now safe to mutate
```

## Alternative: Return Owned Data

If this pattern appears frequently, consider changing the getter to return owned data:

```rust
// Instead of returning &str, return String
fn get_function_shebang(&self, name: &str) -> Option<String> {
    self.function_metadata
        .get(name)
        .and_then(|m| m.shebang.clone())
}
```

Trade-off: Small allocation overhead vs. cleaner call sites.

## When to Apply

This pattern is needed when:
- Adding mutable state tracking to previously stateless methods
- Implementing caching, logging, or capture functionality
- Transitioning from functional to stateful design

## Performance Note

The cloning is typically cheap (small strings like "bash", "python3") and only happens once per function call. Profile before optimizing if concerned.
