use std::collections::HashSet;

fn dfs<F>(
    node: &str,
    get_deps: &F,
    visited: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
    execution_order: &mut Vec<String>,
) -> Result<(), String>
where
    F: Fn(&str) -> Vec<String>,
{
    if visiting.contains(node) {
        // Find the cycle path for the error message
        return Err(format!(
            "Circular dependency detected involving task '{node}'"
        ));
    }
    if visited.contains(node) {
        return Ok(());
    }

    visiting.insert(node.to_string());

    let deps = get_deps(node);
    for dep in deps {
        dfs(&dep, get_deps, visited, visiting, execution_order)?;
    }

    visiting.remove(node);
    visited.insert(node.to_string());
    execution_order.push(node.to_string());

    Ok(())
}

/// Resolve the execution order for a target task based on its dependencies.
/// Returns a topologically sorted list of tasks to execute, or an error if a circular dependency is detected.
///
/// # Errors
///
/// Returns an error if a circular dependency is detected between tasks.
pub fn resolve_dependencies<F>(target: &str, get_deps: F) -> Result<Vec<String>, String>
where
    F: Fn(&str) -> Vec<String>,
{
    let mut execution_order = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    dfs(
        target,
        &get_deps,
        &mut visited,
        &mut visiting,
        &mut execution_order,
    )?;

    Ok(execution_order)
}
