//! Preamble building for function composition
//!
//! This module handles building preambles that inject sibling functions
//! and variables into function execution contexts for composition support.

use super::shell::{escape_pwsh_value, escape_shell_value};
use crate::ast::Attribute;
use crate::transpiler::{self, Interpreter as TranspilerInterpreter};
use std::collections::HashMap;
type InterpreterResolver<'a> =
    dyn Fn(&str, &[Attribute], Option<&str>) -> TranspilerInterpreter + 'a;

/// Collect compatible sibling function names for call site rewriting
pub(super) fn collect_compatible_siblings(
    target_name: &str,
    target_interpreter: &TranspilerInterpreter,
    simple_functions: &HashMap<String, String>,
    block_functions: &HashMap<String, Vec<String>>,
    function_metadata: &HashMap<String, super::FunctionMetadata>,
    resolve_interpreter: &InterpreterResolver<'_>,
) -> Vec<String> {
    let mut compatible = Vec::new();

    // Check simple functions
    for name in simple_functions.keys() {
        if name == target_name {
            continue;
        }
        let metadata = function_metadata.get(name);
        let attributes: &[Attribute] =
            metadata.map_or(&[] as &[Attribute], |m| m.attributes.as_slice());
        let func_interpreter = resolve_interpreter(name, attributes, None);

        if target_interpreter.is_compatible_with(&func_interpreter) {
            compatible.push(name.clone());
        }
    }

    // Check block functions
    for name in block_functions.keys() {
        if name == target_name {
            continue;
        }
        let metadata = function_metadata.get(name);
        let (attributes, shebang) = metadata.map_or_else(
            || (Vec::new(), None),
            |m| (m.attributes.clone(), m.shebang.as_deref()),
        );
        let func_interpreter = resolve_interpreter(name, &attributes, shebang);

        if target_interpreter.is_compatible_with(&func_interpreter) && !compatible.contains(name) {
            compatible.push(name.clone());
        }
    }

    compatible
}

/// Collect incompatible sibling function names (those with colons that need run wrappers)
pub(super) fn collect_incompatible_colon_siblings(
    target_name: &str,
    target_interpreter: &TranspilerInterpreter,
    simple_functions: &HashMap<String, String>,
    block_functions: &HashMap<String, Vec<String>>,
    function_metadata: &HashMap<String, super::FunctionMetadata>,
    resolve_interpreter: &InterpreterResolver<'_>,
) -> Vec<String> {
    let mut incompatible = Vec::new();

    // Check simple functions
    for name in simple_functions.keys() {
        if name == target_name || !name.contains(':') {
            continue;
        }
        let metadata = function_metadata.get(name);
        let attributes: &[Attribute] =
            metadata.map_or(&[] as &[Attribute], |m| m.attributes.as_slice());
        let func_interpreter = resolve_interpreter(name, attributes, None);

        if !target_interpreter.is_compatible_with(&func_interpreter) {
            incompatible.push(name.clone());
        }
    }

    // Check block functions
    for name in block_functions.keys() {
        if name == target_name || !name.contains(':') {
            continue;
        }
        let metadata = function_metadata.get(name);
        let (attributes, shebang) = metadata.map_or_else(
            || (Vec::new(), None),
            |m| (m.attributes.clone(), m.shebang.as_deref()),
        );
        let func_interpreter = resolve_interpreter(name, &attributes, shebang);

        if !target_interpreter.is_compatible_with(&func_interpreter) && !incompatible.contains(name)
        {
            incompatible.push(name.clone());
        }
    }

    incompatible
}

/// Build wrapper functions for incompatible siblings (calls `run <function>`)
fn build_incompatible_wrappers(
    incompatible: &[String],
    target_interpreter: &TranspilerInterpreter,
) -> String {
    if incompatible.is_empty() {
        return String::new();
    }

    let mut wrappers = String::new();

    for name in incompatible {
        let sanitised = transpiler::sanitise_name(name);
        // Convert colon notation to space notation for run command
        // e.g., "node:hello" -> "node hello"
        let run_args = name.replace(':', " ");

        let wrapper = match target_interpreter {
            TranspilerInterpreter::Pwsh => {
                format!("function {sanitised} {{\n    run {run_args} @args\n}}")
            }
            _ => {
                format!("{sanitised}() {{\n    run {run_args} \"$@\"\n}}")
            }
        };

        wrappers.push_str(&wrapper);
        wrappers.push_str("\n\n");
    }

    wrappers
}

/// Build a preamble of all compatible sibling functions
pub(super) fn build_function_preamble(
    target_name: &str,
    target_interpreter: &TranspilerInterpreter,
    simple_functions: &HashMap<String, String>,
    block_functions: &HashMap<String, Vec<String>>,
    function_metadata: &HashMap<String, super::FunctionMetadata>,
    resolve_interpreter: &InterpreterResolver<'_>,
) -> String {
    let mut preamble = String::new();

    // Collect compatible sibling function names
    let compatible_siblings = collect_compatible_siblings(
        target_name,
        target_interpreter,
        simple_functions,
        block_functions,
        function_metadata,
        resolve_interpreter,
    );

    // Also collect incompatible colon siblings so their call sites within
    // compatible preamble functions get rewritten to match the wrapper names
    let incompatible_colon_siblings = collect_incompatible_colon_siblings(
        target_name,
        target_interpreter,
        simple_functions,
        block_functions,
        function_metadata,
        resolve_interpreter,
    );

    // Combine both lists for call site rewriting
    let all_rewritable: Vec<&str> = compatible_siblings
        .iter()
        .chain(incompatible_colon_siblings.iter())
        .map(String::as_str)
        .collect();

    // Transpile simple functions
    for (name, command_template) in simple_functions {
        if name == target_name {
            continue;
        }

        let metadata = function_metadata.get(name);
        let attributes: &[Attribute] =
            metadata.map_or(&[] as &[Attribute], |m| m.attributes.as_slice());
        let func_interpreter = resolve_interpreter(name, attributes, None);

        if !target_interpreter.is_compatible_with(&func_interpreter) {
            continue;
        }

        // Rewrite call sites in the command template
        let rewritten_body = transpiler::rewrite_call_sites(command_template, &all_rewritable);

        let transpiled = match target_interpreter {
            TranspilerInterpreter::Pwsh => {
                transpiler::transpile_to_pwsh(name, &rewritten_body, false)
            }
            _ => transpiler::transpile_to_shell(name, &rewritten_body, false),
        };

        preamble.push_str(&transpiled);
        preamble.push_str("\n\n");
    }

    // Transpile block functions
    for (name, commands) in block_functions {
        if name == target_name {
            continue;
        }

        let metadata = function_metadata.get(name);
        let (attributes, shebang) = metadata.map_or_else(
            || (Vec::new(), None),
            |m| (m.attributes.clone(), m.shebang.as_deref()),
        );
        let func_interpreter = resolve_interpreter(name, &attributes, shebang);

        if !target_interpreter.is_compatible_with(&func_interpreter) {
            continue;
        }

        // Join commands and rewrite call sites
        let body = commands
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        let rewritten_body = transpiler::rewrite_call_sites(&body, &all_rewritable);

        let transpiled = match target_interpreter {
            TranspilerInterpreter::Pwsh => {
                transpiler::transpile_to_pwsh(name, &rewritten_body, true)
            }
            _ => transpiler::transpile_to_shell(name, &rewritten_body, true),
        };

        preamble.push_str(&transpiled);
        preamble.push_str("\n\n");
    }

    // Add wrapper functions for incompatible siblings with colons
    let wrappers = build_incompatible_wrappers(&incompatible_colon_siblings, target_interpreter);
    if !wrappers.is_empty() {
        preamble.push_str(&wrappers);
    }

    preamble
}

/// Build a preamble of variable assignments
pub(super) fn build_variable_preamble(
    variables: &HashMap<String, String>,
    target_interpreter: &TranspilerInterpreter,
) -> String {
    if variables.is_empty() {
        return String::new();
    }

    match target_interpreter {
        TranspilerInterpreter::Pwsh => {
            // PowerShell variable syntax: $VAR = "value"
            variables
                .iter()
                .map(|(k, v)| format!("${} = \"{}\"", k, escape_pwsh_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => {
            // Shell variable syntax: VAR="value"
            variables
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, escape_shell_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_build_variable_preamble_empty() {
        let vars = HashMap::new();
        assert_eq!(
            build_variable_preamble(&vars, &TranspilerInterpreter::Sh),
            ""
        );
    }

    #[test]
    fn test_build_variable_preamble_shell() {
        let mut vars = HashMap::new();
        vars.insert("MY_VAR".to_string(), "hello".to_string());
        let result = build_variable_preamble(&vars, &TranspilerInterpreter::Sh);
        assert_eq!(result, "MY_VAR=\"hello\"");
    }

    #[test]
    fn test_build_variable_preamble_shell_with_special_chars() {
        let mut vars = HashMap::new();
        vars.insert("VAR".to_string(), "say \"hi\"".to_string());
        let result = build_variable_preamble(&vars, &TranspilerInterpreter::Bash);
        assert_eq!(result, "VAR=\"say \\\"hi\\\"\"");
    }

    #[test]
    fn test_build_variable_preamble_pwsh() {
        let mut vars = HashMap::new();
        vars.insert("MY_VAR".to_string(), "hello".to_string());
        let result = build_variable_preamble(&vars, &TranspilerInterpreter::Pwsh);
        assert_eq!(result, "$MY_VAR = \"hello\"");
    }

    #[test]
    fn test_build_variable_preamble_pwsh_with_special_chars() {
        let mut vars = HashMap::new();
        vars.insert("VAR".to_string(), "$env:PATH".to_string());
        let result = build_variable_preamble(&vars, &TranspilerInterpreter::Pwsh);
        assert_eq!(result, "$VAR = \"`$env:PATH\"");
    }

    #[test]
    fn test_collect_compatible_siblings_empty() {
        let simple = HashMap::new();
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = collect_compatible_siblings(
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
    fn test_collect_compatible_siblings_same_interpreter() {
        let mut simple = HashMap::new();
        simple.insert("helper".to_string(), "echo help".to_string());
        simple.insert("target".to_string(), "echo target".to_string());
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = collect_compatible_siblings(
            "target",
            &TranspilerInterpreter::Sh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.contains(&"helper".to_string()));
        assert!(!result.contains(&"target".to_string()));
    }

    #[test]
    fn test_collect_compatible_siblings_with_blocks() {
        let simple = HashMap::new();
        let mut block = HashMap::new();
        block.insert(
            "block_helper".to_string(),
            vec!["echo step1".to_string(), "echo step2".to_string()],
        );
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = collect_compatible_siblings(
            "target",
            &TranspilerInterpreter::Sh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.contains(&"block_helper".to_string()));
    }

    #[test]
    fn test_collect_incompatible_colon_siblings_empty() {
        let simple = HashMap::new();
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = collect_incompatible_colon_siblings(
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
    fn test_collect_incompatible_colon_siblings_finds_colon_functions() {
        let mut simple = HashMap::new();
        simple.insert("node:hello".to_string(), "console.log('hi')".to_string());
        simple.insert("no_colon".to_string(), "echo hi".to_string());
        let block = HashMap::new();
        let mut metadata = HashMap::new();
        metadata.insert(
            "node:hello".to_string(),
            super::super::FunctionMetadata {
                attributes: vec![Attribute::Shell(crate::ast::ShellType::Node)],
                shebang: None,
                params: vec![],
            },
        );
        let resolve = |_name: &str, attrs: &[Attribute], _: Option<&str>| {
            for attr in attrs {
                if let Attribute::Shell(st) = attr {
                    return TranspilerInterpreter::from_shell_type(st);
                }
            }
            TranspilerInterpreter::Sh
        };

        let result = collect_incompatible_colon_siblings(
            "target",
            &TranspilerInterpreter::Sh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.contains(&"node:hello".to_string()));
        assert!(!result.contains(&"no_colon".to_string()));
    }

    #[test]
    fn test_build_function_preamble_empty() {
        let simple = HashMap::new();
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = build_function_preamble(
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
    fn test_build_function_preamble_with_simple_sibling() {
        let mut simple = HashMap::new();
        simple.insert("helper".to_string(), "echo help".to_string());
        simple.insert("target".to_string(), "helper".to_string());
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = build_function_preamble(
            "target",
            &TranspilerInterpreter::Sh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.contains("helper()"));
        assert!(result.contains("echo help"));
    }

    #[test]
    fn test_build_function_preamble_with_block_sibling() {
        let simple = HashMap::new();
        let mut block = HashMap::new();
        block.insert(
            "setup".to_string(),
            vec!["echo step1".to_string(), "echo step2".to_string()],
        );
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = build_function_preamble(
            "target",
            &TranspilerInterpreter::Sh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.contains("setup()"));
        assert!(result.contains("echo step1"));
    }

    #[test]
    fn test_build_function_preamble_excludes_self() {
        let mut simple = HashMap::new();
        simple.insert("target".to_string(), "echo target".to_string());
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Sh;

        let result = build_function_preamble(
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
    fn test_build_function_preamble_pwsh() {
        let mut simple = HashMap::new();
        simple.insert("helper".to_string(), "Write-Host help".to_string());
        simple.insert("target".to_string(), "helper".to_string());
        let block = HashMap::new();
        let metadata = HashMap::new();
        let resolve = |_: &str, _: &[Attribute], _: Option<&str>| TranspilerInterpreter::Pwsh;

        let result = build_function_preamble(
            "target",
            &TranspilerInterpreter::Pwsh,
            &simple,
            &block,
            &metadata,
            &resolve,
        );
        assert!(result.contains("function helper"));
    }
}
