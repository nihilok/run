//! Helper functions for function execution

use crate::ast::Attribute;
use crate::transpiler::Interpreter as TranspilerInterpreter;
use super::FunctionMetadata;
use super::preamble;
use std::collections::HashMap;

/// Collect all rewritable sibling names (compatible + incompatible colon siblings)
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

/// Build the combined script with preambles and body
pub(super) fn build_combined_script(
    var_preamble: String,
    func_preamble: String,
    rewritten_body: String,
) -> String {
    if var_preamble.is_empty() && func_preamble.is_empty() {
        // No preamble needed, just use the body
        rewritten_body
    } else {
        // Build full script with preambles
        let mut parts = Vec::new();
        if !var_preamble.is_empty() {
            parts.push(var_preamble);
        }
        if !func_preamble.is_empty() {
            parts.push(func_preamble);
        }
        parts.push(rewritten_body);
        parts.join("\n")
    }
}

/// Prepare execution attributes for polyglot languages
pub(super) fn prepare_polyglot_attributes(
    attributes: &[Attribute],
    shebang: Option<&str>,
) -> Vec<Attribute> {
    if let Some(attr) = attributes.iter().find(|a| matches!(a, Attribute::Shell(_))) {
        vec![attr.clone()]
    } else if let Some(shebang_str) = shebang {
        if let Some(shell_type) = super::shell::resolve_shebang_interpreter(shebang_str) {
            vec![Attribute::Shell(shell_type)]
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    }
}
