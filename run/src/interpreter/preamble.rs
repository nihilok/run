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

/// Build a preamble that declares named variables from function parameters
/// for polyglot scripts (Python, Node.js, Ruby).
///
/// This allows polyglot functions with named params (e.g., `greet(name, greeting = "Hello")`)
/// to access their arguments as proper named variables instead of manually unpacking
/// `sys.argv`, `process.argv`, etc.
pub(super) fn build_polyglot_arg_preamble(
    params: &[crate::ast::Parameter],
    interpreter: &TranspilerInterpreter,
) -> String {
    if params.is_empty() {
        return String::new();
    }

    let is_polyglot = matches!(
        interpreter,
        TranspilerInterpreter::Python
            | TranspilerInterpreter::Python3
            | TranspilerInterpreter::Node
            | TranspilerInterpreter::Ruby
    );

    if !is_polyglot {
        return String::new();
    }

    match interpreter {
        TranspilerInterpreter::Python | TranspilerInterpreter::Python3 => {
            build_python_arg_preamble(params)
        }
        TranspilerInterpreter::Node => build_node_arg_preamble(params),
        TranspilerInterpreter::Ruby => build_ruby_arg_preamble(params),
        _ => String::new(),
    }
}

/// Build Python variable declarations from parameters.
/// Python uses `sys.argv` where index 0 is `-c`, so user args start at index 1.
fn build_python_arg_preamble(params: &[crate::ast::Parameter]) -> String {
    let needs_json = params
        .iter()
        .any(|p| matches!(p.param_type, crate::ast::ArgType::Object));
    let mut lines = if needs_json {
        vec!["import sys".to_string(), "import json".to_string()]
    } else {
        vec!["import sys".to_string()]
    };

    for (i, param) in params.iter().enumerate() {
        let idx = i + 1; // sys.argv[0] is -c

        if param.is_rest {
            let raw = format!("sys.argv[{idx}:]");
            let converted = convert_python_list(&raw, &param.param_type);
            lines.push(format!("{} = {}", param.name, converted));
        } else if let Some(ref default) = param.default_value {
            let default_literal = python_literal(default, &param.param_type);
            let raw_expr =
                format!("sys.argv[{idx}] if len(sys.argv) > {idx} else {default_literal}");
            // For types needing conversion, wrap the whole expression
            let line = match param.param_type {
                crate::ast::ArgType::Integer => format!(
                    "{} = int(sys.argv[{idx}]) if len(sys.argv) > {idx} else {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::Float => format!(
                    "{} = float(sys.argv[{idx}]) if len(sys.argv) > {idx} else {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::Boolean => format!(
                    "{} = (sys.argv[{idx}].lower() in ('true', '1', 'yes')) if len(sys.argv) > {idx} else {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::Object => format!(
                    "{} = json.loads(sys.argv[{idx}]) if len(sys.argv) > {idx} else {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::String => format!("{} = {}", param.name, raw_expr),
            };
            lines.push(line);
        } else {
            let raw = format!("sys.argv[{idx}]");
            let converted = convert_python_value(&raw, &param.param_type);
            lines.push(format!("{} = {}", param.name, converted));
        }
    }

    lines.join("\n")
}

/// Build Node.js variable declarations from parameters.
/// `process.argv[0]` is the node binary path; user args start at index 1.
fn build_node_arg_preamble(params: &[crate::ast::Parameter]) -> String {
    let mut lines = Vec::new();

    for (i, param) in params.iter().enumerate() {
        let idx = i + 1; // process.argv[0] is node path

        if param.is_rest {
            let raw = format!("process.argv.slice({idx})");
            let converted = convert_node_list(&raw, &param.param_type);
            lines.push(format!("const {} = {};", param.name, converted));
        } else if let Some(ref default) = param.default_value {
            let default_literal = node_literal(default, &param.param_type);
            let line = match param.param_type {
                crate::ast::ArgType::Integer => format!(
                    "const {} = process.argv.length > {idx} ? parseInt(process.argv[{idx}], 10) : {default_literal};",
                    param.name
                ),
                crate::ast::ArgType::Float => format!(
                    "const {} = process.argv.length > {idx} ? parseFloat(process.argv[{idx}]) : {default_literal};",
                    param.name
                ),
                crate::ast::ArgType::Boolean => format!(
                    "const {} = process.argv.length > {idx} ? !['false', '0', ''].includes(process.argv[{idx}].toLowerCase()) : {default_literal};",
                    param.name
                ),
                crate::ast::ArgType::Object => format!(
                    "const {} = process.argv.length > {idx} ? JSON.parse(process.argv[{idx}]) : {default_literal};",
                    param.name
                ),
                crate::ast::ArgType::String => format!(
                    "const {} = process.argv.length > {idx} ? process.argv[{idx}] : {default_literal};",
                    param.name
                ),
            };
            lines.push(line);
        } else {
            let raw = format!("process.argv[{idx}]");
            let converted = convert_node_value(&raw, &param.param_type);
            lines.push(format!("const {} = {};", param.name, converted));
        }
    }

    lines.join("\n")
}

/// Build Ruby variable declarations from parameters.
/// Ruby uses `ARGV` where index 0 is the first user argument.
fn build_ruby_arg_preamble(params: &[crate::ast::Parameter]) -> String {
    let needs_json = params
        .iter()
        .any(|p| matches!(p.param_type, crate::ast::ArgType::Object));
    let mut lines = Vec::new();
    if needs_json {
        lines.push("require 'json'".to_string());
    }

    for (i, param) in params.iter().enumerate() {
        if param.is_rest {
            let raw = format!("ARGV[{i}..]");
            let converted = convert_ruby_list(&raw, &param.param_type);
            lines.push(format!("{} = {}", param.name, converted));
        } else if let Some(ref default) = param.default_value {
            let default_literal = ruby_literal(default, &param.param_type);
            let line = match param.param_type {
                crate::ast::ArgType::Integer => format!(
                    "{} = ARGV.length > {i} ? ARGV[{i}].to_i : {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::Float => format!(
                    "{} = ARGV.length > {i} ? ARGV[{i}].to_f : {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::Boolean => format!(
                    "{} = ARGV.length > {i} ? !['false', '0', ''].include?(ARGV[{i}].downcase) : {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::Object => format!(
                    "{} = ARGV.length > {i} ? JSON.parse(ARGV[{i}]) : {default_literal}",
                    param.name
                ),
                crate::ast::ArgType::String => format!(
                    "{} = ARGV.length > {i} ? ARGV[{i}] : {default_literal}",
                    param.name
                ),
            };
            lines.push(line);
        } else {
            let raw = format!("ARGV[{i}]");
            let converted = convert_ruby_value(&raw, &param.param_type);
            lines.push(format!("{} = {}", param.name, converted));
        }
    }

    lines.join("\n")
}

// --- Python helpers ---

fn python_literal(value: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer | crate::ast::ArgType::Float => value.to_string(),
        crate::ast::ArgType::Boolean => {
            if ["true", "1", "yes"].contains(&value.to_lowercase().as_str()) {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        crate::ast::ArgType::Object => {
            format!(
                "json.loads('{}')",
                value.replace('\\', "\\\\").replace('\'', "\\'")
            )
        }
        crate::ast::ArgType::String => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
    }
}

fn convert_python_value(expr: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer => format!("int({expr})"),
        crate::ast::ArgType::Float => format!("float({expr})"),
        crate::ast::ArgType::Boolean => {
            format!("{expr}.lower() in ('true', '1', 'yes')")
        }
        crate::ast::ArgType::Object => format!("json.loads({expr})"),
        crate::ast::ArgType::String => expr.to_string(),
    }
}

fn convert_python_list(expr: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer => format!("[int(x) for x in {expr}]"),
        crate::ast::ArgType::Float => format!("[float(x) for x in {expr}]"),
        crate::ast::ArgType::Boolean => {
            format!("[x.lower() in ('true', '1', 'yes') for x in {expr}]")
        }
        crate::ast::ArgType::Object => format!("[json.loads(x) for x in {expr}]"),
        crate::ast::ArgType::String => expr.to_string(),
    }
}

// --- Node.js helpers ---

fn node_literal(value: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer | crate::ast::ArgType::Float => value.to_string(),
        crate::ast::ArgType::Boolean => {
            if ["true", "1", "yes"].contains(&value.to_lowercase().as_str()) {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        crate::ast::ArgType::Object => {
            format!(
                "JSON.parse('{}')",
                value.replace('\\', "\\\\").replace('\'', "\\'")
            )
        }
        crate::ast::ArgType::String => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
    }
}

fn convert_node_value(expr: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer => format!("parseInt({expr}, 10)"),
        crate::ast::ArgType::Float => format!("parseFloat({expr})"),
        crate::ast::ArgType::Boolean => {
            format!("!['false', '0', ''].includes({expr}.toLowerCase())")
        }
        crate::ast::ArgType::Object => format!("JSON.parse({expr})"),
        crate::ast::ArgType::String => expr.to_string(),
    }
}

fn convert_node_list(expr: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer => format!("{expr}.map(x => parseInt(x, 10))"),
        crate::ast::ArgType::Float => format!("{expr}.map(x => parseFloat(x))"),
        crate::ast::ArgType::Boolean => {
            format!("{expr}.map(x => !['false', '0', ''].includes(x.toLowerCase()))")
        }
        crate::ast::ArgType::Object => format!("{expr}.map(x => JSON.parse(x))"),
        crate::ast::ArgType::String => expr.to_string(),
    }
}

// --- Ruby helpers ---

fn ruby_literal(value: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer | crate::ast::ArgType::Float => value.to_string(),
        crate::ast::ArgType::Boolean => {
            if ["true", "1", "yes"].contains(&value.to_lowercase().as_str()) {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        crate::ast::ArgType::Object => {
            format!(
                "JSON.parse('{}')",
                value.replace('\\', "\\\\").replace('\'', "\\'")
            )
        }
        crate::ast::ArgType::String => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
    }
}

fn convert_ruby_value(expr: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer => format!("{expr}.to_i"),
        crate::ast::ArgType::Float => format!("{expr}.to_f"),
        crate::ast::ArgType::Boolean => {
            format!("!['false', '0', ''].include?({expr}.downcase)")
        }
        crate::ast::ArgType::Object => format!("JSON.parse({expr})"),
        crate::ast::ArgType::String => expr.to_string(),
    }
}

fn convert_ruby_list(expr: &str, arg_type: &crate::ast::ArgType) -> String {
    match arg_type {
        crate::ast::ArgType::Integer => format!("{expr}.map(&:to_i)"),
        crate::ast::ArgType::Float => format!("{expr}.map(&:to_f)"),
        crate::ast::ArgType::Boolean => {
            format!("{expr}.map {{ |x| !['false', '0', ''].include?(x.downcase) }}")
        }
        crate::ast::ArgType::Object => {
            format!("{expr}.map {{ |x| JSON.parse(x) }}")
        }
        crate::ast::ArgType::String => expr.to_string(),
    }
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

    // --- Polyglot arg preamble tests ---

    fn make_param(name: &str, default: Option<&str>, is_rest: bool) -> crate::ast::Parameter {
        crate::ast::Parameter {
            name: name.to_string(),
            param_type: crate::ast::ArgType::String,
            default_value: default.map(String::from),
            is_rest,
        }
    }

    fn make_typed_param(
        name: &str,
        arg_type: crate::ast::ArgType,
        default: Option<&str>,
    ) -> crate::ast::Parameter {
        crate::ast::Parameter {
            name: name.to_string(),
            param_type: arg_type,
            default_value: default.map(String::from),
            is_rest: false,
        }
    }

    #[test]
    fn test_polyglot_preamble_empty_params() {
        let result = build_polyglot_arg_preamble(&[], &TranspilerInterpreter::Python);
        assert_eq!(result, "");
    }

    #[test]
    fn test_polyglot_preamble_non_polyglot() {
        let params = vec![make_param("name", None, false)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Sh);
        assert_eq!(result, "");
    }

    #[test]
    fn test_python_preamble_required_param() {
        let params = vec![make_param("name", None, false)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("import sys"));
        assert!(result.contains("name = sys.argv[1]"));
    }

    #[test]
    fn test_python_preamble_default_param() {
        let params = vec![
            make_param("name", None, false),
            make_param("greeting", Some("Hello"), false),
        ];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("name = sys.argv[1]"));
        assert!(result.contains("greeting = sys.argv[2] if len(sys.argv) > 2 else \"Hello\""));
    }

    #[test]
    fn test_python_preamble_rest_param() {
        let params = vec![
            make_param("name", None, false),
            make_param("extra", None, true),
        ];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("name = sys.argv[1]"));
        assert!(result.contains("extra = sys.argv[2:]"));
    }

    #[test]
    fn test_python_preamble_integer_type() {
        let params = vec![make_typed_param(
            "count",
            crate::ast::ArgType::Integer,
            None,
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("count = int(sys.argv[1])"));
    }

    #[test]
    fn test_python_preamble_boolean_type() {
        let params = vec![make_typed_param(
            "verbose",
            crate::ast::ArgType::Boolean,
            None,
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("verbose = sys.argv[1].lower() in ('true', '1', 'yes')"));
    }

    #[test]
    fn test_node_preamble_required_param() {
        let params = vec![make_param("name", None, false)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains("const name = process.argv[1];"));
    }

    #[test]
    fn test_node_preamble_default_param() {
        let params = vec![
            make_param("name", None, false),
            make_param("greeting", Some("Hello"), false),
        ];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains("const name = process.argv[1];"));
        assert!(
            result.contains(
                "const greeting = process.argv.length > 2 ? process.argv[2] : \"Hello\";"
            )
        );
    }

    #[test]
    fn test_node_preamble_rest_param() {
        let params = vec![
            make_param("name", None, false),
            make_param("extra", None, true),
        ];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains("const extra = process.argv.slice(2);"));
    }

    #[test]
    fn test_node_preamble_integer_type() {
        let params = vec![make_typed_param(
            "count",
            crate::ast::ArgType::Integer,
            None,
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains("const count = parseInt(process.argv[1], 10);"));
    }

    #[test]
    fn test_ruby_preamble_required_param() {
        let params = vec![make_param("name", None, false)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Ruby);
        assert!(result.contains("name = ARGV[0]"));
    }

    #[test]
    fn test_ruby_preamble_default_param() {
        let params = vec![
            make_param("name", None, false),
            make_param("greeting", Some("Hello"), false),
        ];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Ruby);
        assert!(result.contains("name = ARGV[0]"));
        assert!(result.contains("greeting = ARGV.length > 1 ? ARGV[1] : \"Hello\""));
    }

    #[test]
    fn test_ruby_preamble_rest_param() {
        let params = vec![
            make_param("name", None, false),
            make_param("extra", None, true),
        ];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Ruby);
        assert!(result.contains("extra = ARGV[1..]"));
    }

    #[test]
    fn test_python3_preamble_works() {
        let params = vec![make_param("name", None, false)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python3);
        assert!(result.contains("import sys"));
        assert!(result.contains("name = sys.argv[1]"));
    }

    #[test]
    fn test_python_preamble_integer_default() {
        let params = vec![make_typed_param(
            "count",
            crate::ast::ArgType::Integer,
            Some("42"),
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("int(sys.argv[1]) if len(sys.argv) > 1 else 42"));
    }

    #[test]
    fn test_node_preamble_boolean_default() {
        let params = vec![make_typed_param(
            "verbose",
            crate::ast::ArgType::Boolean,
            Some("false"),
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains("const verbose = process.argv.length > 1 ? !['false', '0', ''].includes(process.argv[1].toLowerCase()) : false;"));
    }

    // --- Float type tests ---

    #[test]
    fn test_python_preamble_float_type() {
        let params = vec![make_typed_param("rate", crate::ast::ArgType::Float, None)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("rate = float(sys.argv[1])"));
    }

    #[test]
    fn test_python_preamble_float_default() {
        let params = vec![make_typed_param(
            "rate",
            crate::ast::ArgType::Float,
            Some("3.14"),
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("float(sys.argv[1]) if len(sys.argv) > 1 else 3.14"));
    }

    #[test]
    fn test_node_preamble_float_type() {
        let params = vec![make_typed_param("rate", crate::ast::ArgType::Float, None)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains("const rate = parseFloat(process.argv[1]);"));
    }

    #[test]
    fn test_node_preamble_float_default() {
        let params = vec![make_typed_param(
            "rate",
            crate::ast::ArgType::Float,
            Some("3.14"),
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains(
            "const rate = process.argv.length > 1 ? parseFloat(process.argv[1]) : 3.14;"
        ));
    }

    #[test]
    fn test_ruby_preamble_float_type() {
        let params = vec![make_typed_param("rate", crate::ast::ArgType::Float, None)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Ruby);
        assert!(result.contains("rate = ARGV[0].to_f"));
    }

    #[test]
    fn test_ruby_preamble_float_default() {
        let params = vec![make_typed_param(
            "rate",
            crate::ast::ArgType::Float,
            Some("3.14"),
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Ruby);
        assert!(result.contains("rate = ARGV.length > 0 ? ARGV[0].to_f : 3.14"));
    }

    // --- Object type tests ---

    #[test]
    fn test_python_preamble_object_type() {
        let params = vec![make_typed_param(
            "config",
            crate::ast::ArgType::Object,
            None,
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(result.contains("import json"));
        assert!(result.contains("config = json.loads(sys.argv[1])"));
    }

    #[test]
    fn test_python_preamble_object_no_json_import_without_object() {
        let params = vec![make_typed_param("name", crate::ast::ArgType::String, None)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Python);
        assert!(!result.contains("import json"));
    }

    #[test]
    fn test_node_preamble_object_type() {
        let params = vec![make_typed_param(
            "config",
            crate::ast::ArgType::Object,
            None,
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Node);
        assert!(result.contains("const config = JSON.parse(process.argv[1]);"));
    }

    #[test]
    fn test_ruby_preamble_object_type() {
        let params = vec![make_typed_param(
            "config",
            crate::ast::ArgType::Object,
            None,
        )];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Ruby);
        assert!(result.contains("require 'json'"));
        assert!(result.contains("config = JSON.parse(ARGV[0])"));
    }

    #[test]
    fn test_ruby_preamble_no_json_require_without_object() {
        let params = vec![make_typed_param("name", crate::ast::ArgType::String, None)];
        let result = build_polyglot_arg_preamble(&params, &TranspilerInterpreter::Ruby);
        assert!(!result.contains("require 'json'"));
    }
}
