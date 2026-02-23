//! Helper functions for function execution

use super::FunctionMetadata;
use super::preamble;
use crate::ast::Attribute;
use crate::transpiler::Interpreter as TranspilerInterpreter;
use std::collections::HashMap;

/// Collect all rewritable sibling names (compatible + incompatible colon siblings)
#[allow(clippy::type_complexity)]
pub(super) fn collect_rewritable_siblings(
    target_name: &str,
    target_interpreter: &TranspilerInterpreter,
    simple_functions: &HashMap<String, String>,
    block_functions: &HashMap<String, Vec<String>>,
    function_metadata: &HashMap<String, FunctionMetadata>,
    resolve_interpreter: &dyn Fn(&str, &[Attribute], Option<&str>) -> TranspilerInterpreter,
) -> Vec<String> {
    let mut rewritable_names = preamble::collect_compatible_siblings(
        target_name,
        target_interpreter,
        simple_functions,
        block_functions,
        function_metadata,
        resolve_interpreter,
    );
    rewritable_names.extend(preamble::collect_incompatible_colon_siblings(
        target_name,
        target_interpreter,
        simple_functions,
        block_functions,
        function_metadata,
        resolve_interpreter,
    ));
    rewritable_names
}

/// Build the combined script with preambles and body.
///
/// When `wrap_in_function` is true, the body is wrapped in a `__run__` shell
/// function so that `return` statements work naturally — Runfile functions use
/// function syntax, so users expect `return` to behave as it does inside a
/// shell function. This should be true for shell interpreters (sh/bash/pwsh)
/// but false for polyglot scripts (Python/Node/Ruby).
pub(super) fn build_combined_script(
    var_preamble: String,
    func_preamble: String,
    rewritten_body: String,
    wrap_in_function: bool,
) -> String {
    let body = if wrap_in_function {
        format!("__run__() {{\n{rewritten_body}\n}}\n__run__ \"$@\"")
    } else {
        rewritten_body
    };

    if var_preamble.is_empty() && func_preamble.is_empty() {
        body
    } else {
        let mut parts = Vec::new();
        if !var_preamble.is_empty() {
            parts.push(var_preamble);
        }
        if !func_preamble.is_empty() {
            parts.push(func_preamble);
        }
        parts.push(body);
        parts.join("\n")
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_build_combined_script_no_preambles() {
        let result =
            build_combined_script(String::new(), String::new(), "echo hello".to_string(), true);
        assert_eq!(result, "__run__() {\necho hello\n}\n__run__ \"$@\"");
    }

    #[test]
    fn test_build_combined_script_var_preamble_only() {
        let result = build_combined_script(
            "MY_VAR=\"value\"".to_string(),
            String::new(),
            "echo $MY_VAR".to_string(),
            true,
        );
        assert_eq!(
            result,
            "MY_VAR=\"value\"\n__run__() {\necho $MY_VAR\n}\n__run__ \"$@\""
        );
    }

    #[test]
    fn test_build_combined_script_func_preamble_only() {
        let result = build_combined_script(
            String::new(),
            "helper() {\n    echo help\n}".to_string(),
            "helper".to_string(),
            true,
        );
        assert_eq!(
            result,
            "helper() {\n    echo help\n}\n__run__() {\nhelper\n}\n__run__ \"$@\""
        );
    }

    #[test]
    fn test_build_combined_script_both_preambles() {
        let result = build_combined_script(
            "VAR=\"x\"".to_string(),
            "fn() { echo; }".to_string(),
            "fn $VAR".to_string(),
            true,
        );
        assert_eq!(
            result,
            "VAR=\"x\"\nfn() { echo; }\n__run__() {\nfn $VAR\n}\n__run__ \"$@\""
        );
    }

    #[test]
    fn test_build_combined_script_wraps_body_with_return() {
        let result = build_combined_script(
            String::new(),
            String::new(),
            "if [ \"$1\" = \"fail\" ]; then\n    return 1\nfi\necho ok".to_string(),
            true,
        );
        assert!(result.contains("__run__() {"));
        assert!(result.contains("return 1"));
        assert!(result.ends_with("__run__ \"$@\""));
    }

    #[test]
    fn test_build_combined_script_no_wrap_for_polyglot() {
        let result = build_combined_script(
            "x = 42".to_string(),
            String::new(),
            "print(x)".to_string(),
            false,
        );
        assert_eq!(result, "x = 42\nprint(x)");
        assert!(!result.contains("__run__"));
    }

    #[test]
    fn test_collect_rewritable_siblings_empty() {
        let simple = HashMap::new();
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::default();

        let result = collect_rewritable_siblings(
            "target",
            &TranspilerInterpreter::Sh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn test_collect_rewritable_siblings_with_compatible() {
        let mut simple = HashMap::new();
        simple.insert("helper".to_string(), "echo help".to_string());
        simple.insert("target".to_string(), "echo target".to_string());
        let block = HashMap::new();
        let mut metadata = HashMap::new();
        metadata.insert(
            "helper".to_string(),
            FunctionMetadata {
                attributes: vec![],
                shebang: None,
                params: vec![],
            },
        );
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = collect_rewritable_siblings(
            "target",
            &TranspilerInterpreter::Sh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.contains(&"helper".to_string()));
        assert!(!result.contains(&"target".to_string())); // Should not include self
    }
}
