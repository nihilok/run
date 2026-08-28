# Runfile Dependency Graph Specification

## Concept
Transform `run` from a procedural script runner into a declarative build system by resolving a Directed Acyclic Graph (DAG) of task dependencies before execution.

## Syntax
Developers declare dependencies using the `# @depends` attribute:

```bash
# @depends build, lint, test
deploy:all() {
    echo "Deploying..."
}

# @depends clean
build() {
    cargo build --release
}
```

## Resolution Strategy (Top-Level Graph)
When a user executes `run deploy:all`:
1. **Parse Phase**: `run` parses the `Runfile` and extracts all functions and their `@depends` attributes into `FunctionMetadata`.
2. **Graph Construction**: The CLI constructs a DAG starting from the target task (`deploy:all`).
3. **Topological Sort**: The DAG is sorted to determine execution order. 
   - `clean` -> `build` -> `lint` -> `test` -> `deploy:all`
4. **Execution**: The tasks are executed sequentially in the sorted order. 
   - *Future Phase*: Tasks at the same depth in the DAG can be executed in parallel using `tokio` (e.g., `lint` and `test`).

## Circular Dependency Detection
During graph construction, if a cycle is detected (e.g., `A` depends on `B`, which depends on `A`), `run` immediately aborts with a clear error:
`Error: Circular dependency detected: A -> B -> A`

## Memoization (Task Deduplication)
If `deploy:all` depends on `build` and `test`, and `test` ALSO depends on `build`, the `build` task is only executed **once**. A `HashSet<String>` tracks completed tasks during the execution pipeline.

## Implementation Steps
1. **Parser Update**: Modify `run/src/parser/attributes.rs` to parse `@depends task1, task2`. (Completed)
2. **AST Update**: Add `Depends(Vec<String>)` to `Attribute` and `FunctionMetadata`. (Completed)
3. **Graph Engine**: Add `run/src/graph.rs` to handle DAG construction, topological sorting, and cycle detection.
4. **CLI Integration**: Modify `run_function_call` in `executor.rs` to invoke the `graph::execute_target(target_name)` which will iteratively execute the resolved tasks in order.

