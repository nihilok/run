# Code Quality Improvement Checklist

**Status Tracking for run-rust codebase compliance with coding guidelines**

## Legend
- ✅ Complete
- 🔄 In Progress  
- 🔴 Not Started
- ⏸️ Blocked
- 🟡 Optional

---

## Phase 1: Critical Fixes ✅ COMPLETE

### Error Handling & Lints
- [✅] Add Clippy lint configuration to Cargo.toml
  - [✅] Add `pedantic = { level = "warn", priority = -1 }`
  - [✅] Add `unwrap_used = "deny"`
  - [✅] Add `expect_used = "deny"`

- [✅] Fix unwrap/expect in library code (4 violations)
  - [✅] src/mcp.rs:185 - expect() → error handling
  - [✅] src/mcp.rs:266 - unwrap() → map_err()
  - [✅] src/repl.rs:48 - unwrap() → if let Err
  - [✅] src/transpiler.rs:144 - unwrap() → if let Some

- [✅] Allow unwrap/expect in test code (6 files)
  - [✅] src/parser.rs test module
  - [✅] src/mcp.rs test module
  - [✅] src/transpiler.rs test module
  - [✅] tests/integration_test.rs
  - [✅] tests/rfc003_mcp_test.rs
  - [✅] tests/rfc005_composition_test.rs

- [✅] Verify build and tests
  - [✅] cargo build --release
  - [✅] cargo test (156 tests)
  - [✅] cargo clippy --all-targets

---

## Phase 2: Module Refactoring 🔴 TODO

### Interpreter Module (848 lines → ~200 lines each)
- [ ] Create `src/interpreter/` directory
- [ ] Create `src/interpreter/mod.rs`
  - [ ] Move core Interpreter struct
  - [ ] Move new(), execute(), list_available_functions()
  - [ ] Add pub use re-exports for backward compatibility
  
- [ ] Create `src/interpreter/execution.rs`
  - [ ] Move execute_simple_function()
  - [ ] Move execute_block_commands()
  - [ ] Move call_function_without_parens()
  - [ ] Move call_function_with_args()
  - [ ] Move substitute_args()
  
- [ ] Create `src/interpreter/preamble.rs`
  - [ ] Move build_function_preamble()
  - [ ] Move build_variable_preamble()
  - [ ] Move collect_compatible_siblings()
  - [ ] Move collect_incompatible_colon_siblings()
  - [ ] Move build_incompatible_wrappers()
  
- [ ] Create `src/interpreter/shell.rs`
  - [ ] Move execute_single_shell_invocation()
  - [ ] Move execute_command()
  - [ ] Move execute_command_with_args()
  - [ ] Move resolve_function_interpreter()
  - [ ] Move resolve_shebang_interpreter()
  - [ ] Move strip_shebang()
  - [ ] Move get_python_executable()
  - [ ] Move escape_shell_value()
  - [ ] Move escape_pwsh_value()

- [ ] Update imports in dependent files
  - [ ] src/lib.rs
  - [ ] src/executor.rs
  - [ ] src/repl.rs
  
- [ ] Test after refactor
  - [ ] cargo test
  - [ ] cargo clippy

### Parser Module (694 lines → ~150-200 lines each)
- [ ] Create `src/parser/` directory
- [ ] Create `src/parser/mod.rs`
  - [ ] Move parse_script()
  - [ ] Move parse_statement()
  - [ ] Move parse_command()
  - [ ] Add pub use re-exports
  
- [ ] Create `src/parser/attributes.rs`
  - [ ] Move parse_attributes_from_lines()
  - [ ] Move parse_attribute_line()
  - [ ] Move parse_arg_attribute()
  - [ ] Move strip_quotes()
  
- [ ] Create `src/parser/preprocessing.rs`
  - [ ] Move preprocess_escaped_newlines()
  
- [ ] Create `src/parser/shebang.rs`
  - [ ] Move parse_shebang()
  - [ ] Consider moving strip_shebang (or keep in interpreter)

- [ ] Update imports in dependent files
  - [ ] src/lib.rs
  - [ ] src/executor.rs
  - [ ] src/mcp.rs
  
- [ ] Test after refactor
  - [ ] cargo test
  - [ ] cargo clippy

### MCP Module (660 lines → ~150-200 lines each)
- [ ] Create `src/mcp/` directory
- [ ] Create `src/mcp/mod.rs`
  - [ ] Move serve_mcp()
  - [ ] Move process_request()
  - [ ] Move print_inspect()
  - [ ] Add pub use re-exports
  
- [ ] Create `src/mcp/tools.rs`
  - [ ] Move Tool, InputSchema, ParameterSchema structs
  - [ ] Move InspectOutput struct
  - [ ] Move extract_function_metadata()
  - [ ] Move inspect()
  
- [ ] Create `src/mcp/handlers.rs`
  - [ ] Move JsonRpcRequest, JsonRpcResponse, JsonRpcError structs
  - [ ] Move ServerCapabilities, ServerInfo structs
  - [ ] Move handle_initialize()
  - [ ] Move handle_tools_list()
  - [ ] Move handle_tools_call()
  
- [ ] Create `src/mcp/mapping.rs`
  - [ ] Move map_arguments_to_positional()
  - [ ] Move resolve_tool_name()

- [ ] Update imports in dependent files
  - [ ] src/lib.rs
  - [ ] src/main.rs
  
- [ ] Test after refactor
  - [ ] cargo test
  - [ ] cargo clippy

---

## Phase 3: Function Extraction 🔴 TODO

### Interpreter Functions
- [ ] Extract from execute_simple_function() (~75 lines → ~30-40 lines)
  - [ ] Extract build_combined_script()
  - [ ] Extract collect_rewritable_siblings()
  
- [ ] Extract from execute_block_commands() (~120 lines → ~40-50 lines)
  - [ ] Extract handle_polyglot_execution()
  - [ ] Extract handle_shell_composition()
  - [ ] Extract prepare_execution_context()
  
- [ ] Extract from execute_command_with_args() (~60 lines → ~30-40 lines)
  - [ ] Extract determine_shell_command()
  - [ ] Extract setup_command_args()
  - [ ] Extract execute_with_shell()

- [ ] Test after each extraction
  - [ ] cargo test
  - [ ] cargo clippy

### Parser Functions
- [ ] Extract from parse_script() (~55 lines → ~30-40 lines)
  - [ ] Extract process_program_items()
  - [ ] Extract preprocess_and_parse()
  
- [ ] Extract from parse_statement() (~120 lines → ~40-50 lines)
  - [ ] Extract parse_assignment()
  - [ ] Extract parse_function_def()
  - [ ] Extract parse_block_body()
  - [ ] Extract parse_function_call()

- [ ] Test after each extraction
  - [ ] cargo test
  - [ ] cargo clippy

---

## Phase 4: Test Organization 🔴 TODO

### Split integration_test.rs (2627 lines)
- [ ] Create tests/ structure:
  ```
  tests/
  ├── common/
  │   └── mod.rs           # Shared helpers
  ├── basic_functions.rs   # ~500 lines
  ├── attributes.rs        # ~600 lines
  ├── polyglot.rs          # ~400 lines
  ├── cli.rs               # ~300 lines
  ├── rfc003_mcp_test.rs   # Keep as-is (490 lines)
  └── rfc005_composition_test.rs  # Keep as-is (660 lines)
  ```

- [ ] Move shared helpers to tests/common/mod.rs
  - [ ] get_binary_path()
  - [ ] create_temp_dir()
  - [ ] create_runfile()
  - [ ] is_python_available(), is_node_available(), is_ruby_available()

- [ ] Create tests/basic_functions.rs
  - [ ] Move simple function definition tests
  - [ ] Move function call tests
  - [ ] Move variable substitution tests
  - [ ] Move basic error handling tests

- [ ] Create tests/attributes.rs
  - [ ] Move @os attribute tests
  - [ ] Move @shell attribute tests
  - [ ] Move @desc attribute tests
  - [ ] Move @arg attribute tests
  - [ ] Move platform filtering tests

- [ ] Create tests/polyglot.rs
  - [ ] Move Python interpreter tests
  - [ ] Move Node interpreter tests
  - [ ] Move Ruby interpreter tests
  - [ ] Move shebang detection tests

- [ ] Create tests/cli.rs
  - [ ] Move --version tests
  - [ ] Move --list tests
  - [ ] Move --help tests
  - [ ] Move --inspect tests
  - [ ] Move --generate-completion tests

- [ ] Verify all tests still pass
  - [ ] cargo test (should still be 156 tests)
  - [ ] Verify no duplicate tests
  - [ ] Verify no missing tests

---

## Phase 5: Documentation 🔴 TODO

### Add Error Documentation
- [ ] Add `# Errors` section to public functions
  - [ ] interpreter.rs:50 - `pub fn execute()`
  - [ ] interpreter.rs:88 - `pub fn call_function_without_parens()`
  - [ ] interpreter.rs:147 - `pub fn call_function_with_args()`
  - [ ] parser.rs:212 - `pub fn parse_script()`
  - [ ] executor.rs functions with Result return types
  - [ ] mcp.rs handler functions

### Module Documentation
- [ ] Add/update module-level docs
  - [ ] src/interpreter/mod.rs
  - [ ] src/parser/mod.rs
  - [ ] src/mcp/mod.rs
  - [ ] All submodules

### Verify Documentation
- [ ] cargo doc --no-deps --open
- [ ] Check for missing doc warnings
- [ ] Fix any broken doc links

---

## Phase 6: Performance Optimization 🟡 OPTIONAL

### Profiling
- [ ] Install cargo-flamegraph: `cargo install flamegraph`
- [ ] Profile with representative workload
- [ ] Identify actual bottlenecks
- [ ] Document findings

### Clone Reduction (if needed)
- [ ] Profile clone overhead in interpreter.rs
- [ ] Convert to Cow<'a, [Attribute]> if beneficial
- [ ] Use &str returns instead of String where possible
- [ ] Benchmark before/after changes

### Collection Optimization (if needed)
- [ ] Add with_capacity() where size known
- [ ] Use SmallVec for small collections
- [ ] Benchmark impact

---

## Quality Gates (Run After Each Phase)

### Build Check
```bash
cargo build --release
# Must succeed without errors
```

### Test Check
```bash
cargo test
# All 156+ tests must pass
```

### Lint Check
```bash
cargo clippy --all-targets -- -D warnings
# Should have 0 errors (warnings OK from pedantic)
```

### Format Check
```bash
cargo fmt --check
# Should require no changes
```

---

## Progress Tracking

| Phase | Tasks | Complete | Status |
|-------|-------|----------|--------|
| Phase 1 | 14 | 14 | ✅ 100% |
| Phase 2 | 30 | 0 | 🔴 0% |
| Phase 3 | 14 | 0 | 🔴 0% |
| Phase 4 | 14 | 0 | 🔴 0% |
| Phase 5 | 9 | 0 | 🔴 0% |
| Phase 6 | 8 | 0 | 🟡 Optional |
| **Total** | **89** | **14** | **16%** |

---

## Notes

- Always run tests after each change
- Commit after each successful phase
- Create PRs for review after major phases
- Update this checklist as work progresses
- Document any deviations from plan

---

**Last Updated:** January 17, 2026  
**Next Review:** After Phase 2 completion
