# Cargo Workspace Restructuring for Dual-Package Publishing

**Date:** January 18, 2026  
**Objective:** Support publishing the same codebase under two crate names (`run` and `runtool`) from a single repository.

## Executive Summary

Successfully restructured the repository from a single-crate structure to a Cargo workspace that supports publishing under two different package names. Both `run` and `runtool` install the identical `run` binary, with `runtool` serving as an alternative package name on crates.io.

## Why Dual Package Names?

The `runtool` name serves several purposes:
1. **Availability**: `run` may already be taken on crates.io
2. **Discoverability**: "runtool" is more descriptive and searchable
3. **Marketing**: Emphasizes the "bridge between human and AI tooling" positioning
4. **SEO**: Better matches search terms like "rust task runner tool"

Users can install via either:
```bash
cargo install run
# or
cargo install runtool
```

Both commands install the same `run` binary.

## Architecture

### Workspace Structure

```
run-rust/
├── Cargo.toml                  # Workspace root configuration
├── Cargo.lock                  # Shared dependency lock file
├── run/                        # Main crate
│   ├── Cargo.toml              # Uses workspace dependencies
│   ├── README.md               # Full documentation for crates.io
│   ├── completions/            # Shell completion scripts (included in package)
│   │   ├── run.bash
│   │   ├── run.zsh
│   │   ├── run.fish
│   │   └── run.ps1
│   ├── src/
│   │   ├── lib.rs              # Library exports
│   │   ├── main.rs             # Binary entry point
│   │   ├── cli.rs              # Shared CLI logic (NEW)
│   │   ├── ast.rs
│   │   ├── parser/
│   │   ├── interpreter/
│   │   └── ...
│   └── tests/                  # Integration tests
└── runtool/                    # Wrapper crate
    ├── Cargo.toml              # Depends on run crate
    ├── README.md               # Brief wrapper docs
    └── src/
        └── main.rs             # Delegates to run::cli::run_cli()
```

### Key Design Decisions

#### 1. Shared CLI Module (`run/src/cli.rs`)

**Problem**: Both binaries need identical CLI behavior, but Rust doesn't allow calling `main()` from external crates.

**Solution**: Extracted CLI logic into a public `cli` module:

```rust
// run/src/cli.rs
pub fn run_cli() {
    let cli = Cli::parse();
    // ... all CLI logic
}

// run/src/main.rs
fn main() {
    run::cli::run_cli();
}

// runtool/src/main.rs
fn main() {
    run::cli::run_cli();
}
```

**Benefits**:
- Zero code duplication
- Single source of truth for CLI behavior
- Both binaries guaranteed identical
- Easy to maintain and test

#### 2. Workspace Dependencies

Centralized all dependencies in workspace root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["run", "runtool"]

[workspace.package]
version = "0.3.1"
edition = "2024"
authors = ["nihilok nihilok@jarv.dev", "sebastian <s@porto5.com>"]
license = "MIT"
repository = "https://github.com/nihilok/run"

[workspace.dependencies]
pest = "2.8.5"
clap = { version = "4.5", features = ["derive"] }
# ... etc
```

Member crates reference workspace deps:

```toml
# run/Cargo.toml
[dependencies]
pest.workspace = true
clap.workspace = true
```

**Benefits**:
- Single version for all deps
- Easier dependency updates
- Smaller Cargo.lock
- Consistent across packages

#### 3. README Strategy

Each crate needs its own README for crates.io:

- **`run/README.md`** - Full documentation (copy of main README)
- **`runtool/README.md`** - Compelling intro + pointer to full docs
- **`README.md`** (root) - GitHub repository view

**Why separate READMEs?**: crates.io displays each crate's README independently. Users searching for either package need context-appropriate documentation.

#### 4. Resource Path Updates

Moved code into subdirectory required updating embedded resource paths:

```rust
// Before (single crate)
const BASH_COMPLETION: &str = include_str!("../completions/run.bash");

// After (workspace member)
const BASH_COMPLETION: &str = include_str!("../../completions/run.bash");
```

Resources stay at workspace root to avoid duplication.

## Implementation Steps

### Step 1: Create Workspace Structure

```bash
cd run-rust
mkdir -p run runtool/src
mv src run/
mv Cargo.toml run/
mv Cargo.lock run/
```

### Step 2: Create Workspace Root `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = ["run", "runtool"]

[workspace.package]
version = "0.3.1"
# ... shared metadata

[workspace.dependencies]
# ... all dependencies

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
```

### Step 3: Update Run Crate `Cargo.toml`

```toml
[package]
name = "run"
version.workspace = true
edition.workspace = true
authors.workspace = true
# ... etc

[dependencies]
pest.workspace = true
# ... reference workspace deps

[lints]
workspace = true
```

### Step 4: Extract CLI Logic

Create `run/src/cli.rs`:
```rust
pub fn run_cli() {
    // Move all CLI logic from main.rs here
}
```

Update `run/src/lib.rs`:
```rust
pub mod cli;
```

Update `run/src/main.rs`:
```rust
fn main() {
    run::cli::run_cli();
}
```

### Step 5: Create Runtool Wrapper

`runtool/Cargo.toml`:
```toml
[package]
name = "runtool"
version.workspace = true
# ... workspace metadata

[dependencies]
run = { path = "../run" }

[[bin]]
name = "run"
path = "src/main.rs"
```

`runtool/src/main.rs`:
```rust
fn main() {
    run::cli::run_cli();
}
```

### Step 6: Fix Resource Paths

Update any `include_str!()` calls to use `../../` instead of `../`.

### Step 7: Move Tests

```bash
mv tests run/tests
```

### Step 8: Copy README

```bash
cp README.md run/README.md
```

Create compelling `runtool/README.md` with key value props.

### Step 9: Clean Up Lock Files

```bash
rm -f run/Cargo.lock
cargo build  # Generates workspace Cargo.lock at root
```

## Testing & Verification

### Build Verification
```bash
cargo build                    # Build entire workspace
cargo build -p run             # Build specific package
cargo build -p runtool
cargo build --release          # Release builds
```

### Test Verification
```bash
cargo test                     # Run all tests
cargo test -p run              # Test specific package
```

### Functional Verification
```bash
cargo run -p run -- --help
cargo run -p runtool -- --help
# Both should produce identical output
```

### Package Verification
```bash
cd run
cargo package --list --allow-dirty

cd ../runtool
cargo package --list --allow-dirty
```

## Results

✅ **Build**: Both packages build successfully  
✅ **Tests**: All 76+ tests passing (0 failures)  
✅ **Binary Collision Warnings**: Expected and harmless  
✅ **Functional Equivalence**: Both binaries produce identical output  
✅ **Package Contents**: All necessary files included  

## Publishing Process

### Prerequisites

1. Ensure all tests pass: `cargo test --all`
2. Ensure clean build: `cargo build --release`
3. Update version in workspace `Cargo.toml`
4. Commit all changes
5. Tag release: `git tag v0.x.y`

### Publish Order

**Important**: Publish `run` before `runtool` because runtool depends on the published version of run.

```bash
# 1. Publish run crate
cd run
cargo publish

# 2. Wait for crates.io indexing (~5 minutes)

# 3. Publish runtool crate
cd ../runtool
cargo publish
```

### Post-Publishing

1. Push tags: `git push --tags`
2. Create GitHub release
3. Update documentation if needed
4. Announce on social media / forums

## Common Issues & Solutions

### Issue: Binary filename collision warnings

**Symptom**:
```
warning: output filename collision at /target/debug/run
```

**Cause**: Both packages produce a binary named `run`

**Solution**: This is expected and harmless. Both packages are designed to install the same binary.

### Issue: Resource path errors after restructuring

**Symptom**:
```
error: couldn't read `run/src/../completions/run.bash`
```

**Cause**: Relative paths need updating after moving code into subdirectory

**Solution**: Change `../` to `../../` in all `include_str!()` calls:
```rust
include_str!("../../completions/run.bash")
```

### Issue: Tests not found after workspace creation

**Symptom**: Tests that previously passed now can't be found

**Cause**: Tests remained at workspace root instead of moving to crate

**Solution**: Move tests into the appropriate crate:
```bash
mv tests run/tests
```

### Issue: Cargo.lock conflicts

**Symptom**: Multiple Cargo.lock files, Git conflicts

**Cause**: Workspace should have single lock file at root

**Solution**:
```bash
rm -f run/Cargo.lock runtool/Cargo.lock
cargo build  # Generates workspace lock file
```

### Issue: runtool can't find run::cli module

**Symptom**:
```
error[E0433]: failed to resolve: could not find `cli` in `run`
```

**Cause**: CLI module not exposed in `run/src/lib.rs`

**Solution**: Add to `run/src/lib.rs`:
```rust
pub mod cli;
```

## Benefits Achieved

### 1. Dual Distribution
Users can discover and install via whichever package name they find:
- Search for "run" → find `run` crate
- Search for "runtool" → find `runtool` crate
Both install the same tool.

### 2. Zero Code Duplication
All logic lives in `run/src/`. The `runtool` wrapper is literally 3 lines of code.

### 3. Single Source of Truth
One codebase, one test suite, one set of dependencies. Changes automatically apply to both packages.

### 4. Independent Publishing
Both packages can have independent metadata (description, keywords) for better discoverability while sharing code.

### 5. Maintainability
Workspace dependency management means updating a dependency happens once, applies everywhere.

### 6. Backward Compatibility
Existing users of the `run` crate are unaffected. No breaking changes required.

## Lessons Learned

### 1. Extract Reusable Entry Points Early

If you might ever need to wrap your binary, expose the main logic as a public function from the start:

```rust
// In lib.rs from day one
pub mod cli;

// In main.rs
fn main() {
    your_crate::cli::run_cli();
}
```

This makes creating wrappers trivial.

### 2. Workspace Dependencies Are Worth It

Even for small projects, workspace dependencies:
- Prevent version conflicts
- Make updates easier
- Reduce Cargo.lock size
- Enforce consistency

Set them up from the beginning if you anticipate multiple crates.

### 3. Each Crate Needs Its Own README

crates.io displays crate-specific READMEs. Don't skimp on the wrapper crate's README—it's often the first impression for users who discover that package name first.

### 4. Resource Paths Need Attention

When restructuring, search for ALL file-system operations:
- `include_str!()`
- `include_bytes!()`
- `file!()`
- `concat!()` with paths
- Any hardcoded paths

Update them all consistently.

### 5. Test in Both Contexts

Don't just test the main crate—actually run the wrapper crate too:
```bash
cargo run -p runtool -- --help
```

Ensures the delegation works correctly.

### 6. Binary Name Collisions Are Fine

The warnings are scary but harmless when intentional. Document this in your workspace README so contributors don't panic.

## Future Considerations

### Additional Package Names

The pattern is extensible. If you later want a third package name (e.g., `run-cli`):

1. Add to workspace members
2. Create minimal wrapper
3. Publish

No changes to core code needed.

### Version Management

Currently using workspace-wide versioning. Could switch to independent versioning per package if needed:

```toml
# run/Cargo.toml
version = "1.0.0"

# runtool/Cargo.toml  
version = "1.0.0"
dependencies.run = "1.0"
```

### Library API Exposure

If users might want to use `run` as a library (not just a binary), consider exposing more than just `cli` module:

```rust
// run/src/lib.rs
pub mod ast;
pub mod parser;
pub mod interpreter;
pub mod cli;
```

Then other crates could depend on `run` for programmatic use.

## References

- [Cargo Workspaces Documentation](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Publishing on crates.io](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Workspace Dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#inheriting-a-dependency-from-a-workspace)

## Appendix: File Checklist

When restructuring to a workspace, ensure you handle:

- [ ] Move source code to subdirectory
- [ ] Create workspace root `Cargo.toml`
- [ ] Update member crate `Cargo.toml` files
- [ ] Extract shared CLI logic
- [ ] Create wrapper crate structure
- [ ] Update resource paths (`include_str!`, etc.)
- [ ] Move tests to appropriate crate
- [ ] Copy/create READMEs for each crate
- [ ] Remove duplicate `Cargo.lock` files
- [ ] Update `.gitignore` if needed
- [ ] Update CI/CD configurations
- [ ] Update documentation
- [ ] Test both packages build
- [ ] Test both packages run correctly
- [ ] Verify package contents with `cargo package --list`
- [ ] Update version in workspace root
- [ ] Tag and publish

---

**Status**: ✅ Complete  
**Date Completed**: January 18, 2026  
**Packages Ready for Publishing**: `run` v0.3.1, `runtool` v0.3.1
