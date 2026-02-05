//! Helper functions for function execution

use super::FunctionMetadata;
use super::preamble;
use crate::ast::Attribute;
use crate::transpiler::Interpreter as TranspilerInterpreter;
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
