//! Preamble building for function composition
//!
//! This module handles building preambles that inject sibling functions
//! and variables into function execution contexts for composition support.

use super::shell::{escape_pwsh_value, escape_shell_value};
use crate::ast::Attribute;
use crate::transpiler::{self, Interpreter as TranspilerInterpreter};
use std::collections::HashMap;

/// Collect compatible sibling function names for call site rewriting
pub(super) fn collect_compatible_siblings(
    target_name: &str,
    target_interpreter: &TranspilerInterpreter,
    simple_functions: &HashMap<String, String>,
    block_functions: &HashMap<String, Vec<String>>,
    function_metadata: &HashMap<String, super::FunctionMetadata>,
    resolve_interpreter: &dyn Fn(&str, &[Attribute], Option<&str>) -> TranspilerInterpreter,
) -> Vec<String> {
    let mut compatible = Vec::new();

    // Check simple functions
    for name in simple_functions.keys() {
        if name == target_name {
            continue;
        }
        let metadata = function_metadata.get(name);
        let attributes = metadata.map(|m| m.attributes.as_slice()).unwrap_or(&[]);
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

        if target_interpreter.is_compatible_with(&func_interpreter) {
            if !compatible.contains(name) {
                compatible.push(name.clone());
            }
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
    resolve_interpreter: &dyn Fn(&str, &[Attribute], Option<&str>) -> TranspilerInterpreter,
) -> Vec<String> {
    let mut incompatible = Vec::new();

    // Check simple functions
    for name in simple_functions.keys() {
        if name == target_name || !name.contains(':') {
            continue;
        }
        let metadata = function_metadata.get(name);
        let attributes = metadata.map(|m| m.attributes.as_slice()).unwrap_or(&[]);
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

        if !target_interpreter.is_compatible_with(&func_interpreter) {
            if !incompatible.contains(name) {
                incompatible.push(name.clone());
            }
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
    resolve_interpreter: &dyn Fn(&str, &[Attribute], Option<&str>) -> TranspilerInterpreter,
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
        .map(|s| s.as_str())
        .collect();

    // Transpile simple functions
    for (name, command_template) in simple_functions {
        if name == target_name {
            continue;
        }

        let metadata = function_metadata.get(name);
        let attributes = metadata.map(|m| m.attributes.as_slice()).unwrap_or(&[]);
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
        let body = commands.join("\n");
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
