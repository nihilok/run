//! Dependency graph resolution and topological sorting for tasks
//!
//! Provides DAG construction, cycle detection, and stage-by-stage dependency resolution
//! enabling parallel execution of independent tasks at the same depth in the graph.

use std::collections::{HashMap, HashSet};

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

fn compute_level<F>(node: &str, get_deps: &F, levels: &mut HashMap<String, usize>) -> usize
where
    F: Fn(&str) -> Vec<String>,
{
    if let Some(&lvl) = levels.get(node) {
        return lvl;
    }

    let deps = get_deps(node);
    let lvl = if deps.is_empty() {
        0
    } else {
        deps.iter()
            .map(|dep| compute_level(dep, get_deps, levels))
            .max()
            .map_or(0, |m| m + 1)
    };

    levels.insert(node.to_string(), lvl);
    lvl
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

/// Resolve the execution stages for a target task based on its dependencies.
///
/// Each stage contains tasks that have no dependencies on each other and can safely execute
/// in parallel. Stages are ordered such that all dependencies for tasks in stage N are
/// guaranteed to have finished in stages < N. The final stage always contains only the target task.
///
/// # Errors
///
/// Returns an error if a circular dependency is detected between tasks.
pub fn resolve_dependency_stages<F>(target: &str, get_deps: F) -> Result<Vec<Vec<String>>, String>
where
    F: Fn(&str) -> Vec<String>,
{
    let mut execution_order = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    // 1. Cycle detection and reachability collection via DFS
    dfs(
        target,
        &get_deps,
        &mut visited,
        &mut visiting,
        &mut execution_order,
    )?;

    // 2. Compute the dependency level (distance from leaf nodes) for each reachable task
    let mut levels = HashMap::new();
    for node in &visited {
        compute_level(node, &get_deps, &mut levels);
    }

    // 3. Group tasks by level into execution stages
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut stages = vec![Vec::new(); max_level + 1];

    for (node, level) in levels {
        stages[level].push(node);
    }

    // Sort task names within each stage for deterministic ordering
    for stage in &mut stages {
        stage.sort();
    }

    Ok(stages)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_stages_single_task() {
        let stages = resolve_dependency_stages("build", |_| vec![]).unwrap();
        assert_eq!(stages, vec![vec!["build".to_string()]]);
    }

    #[test]
    fn test_resolve_stages_linear() {
        // deploy -> build -> clean
        let deps = |name: &str| match name {
            "deploy" => vec!["build".to_string()],
            "build" => vec!["clean".to_string()],
            _ => vec![],
        };

        let stages = resolve_dependency_stages("deploy", deps).unwrap();
        assert_eq!(
            stages,
            vec![
                vec!["clean".to_string()],
                vec!["build".to_string()],
                vec!["deploy".to_string()],
            ]
        );
    }

    #[test]
    fn test_resolve_stages_diamond() {
        // a -> b, c
        // b -> d
        // c -> d
        // d -> []
        let deps = |name: &str| match name {
            "a" => vec!["b".to_string(), "c".to_string()],
            "b" | "c" => vec!["d".to_string()],
            _ => vec![],
        };

        let stages = resolve_dependency_stages("a", deps).unwrap();
        assert_eq!(
            stages,
            vec![
                vec!["d".to_string()],
                vec!["b".to_string(), "c".to_string()],
                vec!["a".to_string()],
            ]
        );
    }

    #[test]
    fn test_resolve_stages_unbalanced() {
        // a -> b, c
        // b -> b1 -> b2
        // c -> []
        let deps = |name: &str| match name {
            "a" => vec!["b".to_string(), "c".to_string()],
            "b" => vec!["b1".to_string()],
            "b1" => vec!["b2".to_string()],
            _ => vec![],
        };

        let stages = resolve_dependency_stages("a", deps).unwrap();
        assert_eq!(
            stages,
            vec![
                vec!["b2".to_string(), "c".to_string()],
                vec!["b1".to_string()],
                vec!["b".to_string()],
                vec!["a".to_string()],
            ]
        );
    }

    #[test]
    fn test_resolve_stages_cycle_detected() {
        let deps = |name: &str| match name {
            "a" => vec!["b".to_string()],
            "b" => vec!["a".to_string()],
            _ => vec![],
        };

        let err = resolve_dependency_stages("a", deps).unwrap_err();
        assert!(err.contains("Circular dependency detected"));
    }
}
