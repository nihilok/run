//! Argument mapping and tool name resolution

use super::handlers::JsonRpcError;
use crate::ast::{Attribute, Statement};
use crate::{config, parser};
use std::collections::HashMap;

/// Resolve a sanitised tool name back to the original function name
/// This is needed because MCP requires [a-zA-Z0-9_-] but we support colons in function names
/// Uses merged global+project config to ensure all exposed tools are resolvable
pub(super) fn resolve_tool_name(sanitised_name: &str) -> Result<String, JsonRpcError> {
    let (config_content, _metadata) = config::load_merged_config().ok_or_else(|| JsonRpcError {
        code: -32603,
        message: "No Runfile found".to_string(),
        data: None,
    })?;

    let program = parser::parse_script(&config_content).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Parse error: {e}"),
        data: None,
    })?;

    // Look for a function whose sanitised name matches
    // Process in reverse order (project overrides global, like in inspect())
    let mut matching_name: Option<String> = None;
    for statement in program.statements.iter().rev() {
        let (name, attributes) = match statement {
            Statement::SimpleFunctionDef {
                name, attributes, ..
            }
            | Statement::BlockFunctionDef {
                name, attributes, ..
            } => (name, attributes),
            _ => continue,
        };

        // Check if this function has @desc (would be exposed as tool)
        if attributes.iter().any(|a| matches!(a, Attribute::Desc(_))) {
            let tool_name = name.replace(':', "__");
            if tool_name == sanitised_name && matching_name.is_none() {
                // Found a match; since we're processing in reverse, this is the project version
                matching_name = Some(name.clone());
                break;
            }
        }
    }

    matching_name.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: format!("Tool not found: {sanitised_name}"),
        data: None,
    })
}

/// Map JSON arguments to positional shell arguments
/// Uses merged global+project config with project taking precedence
pub(super) fn map_arguments_to_positional(
    tool_name: &str,
    json_args: &serde_json::Value,
) -> Result<Vec<String>, JsonRpcError> {
    // Load merged Runfile to get function metadata
    let (config_content, _metadata) = config::load_merged_config().ok_or_else(|| JsonRpcError {
        code: -32603,
        message: "No Runfile found".to_string(),
        data: None,
    })?;

    let program = parser::parse_script(&config_content).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Parse error: {e}"),
        data: None,
    })?;

    // Find the function and get its @arg attributes and parameters
    // Process in reverse order to give precedence to project definitions
    let mut arg_mapping: HashMap<usize, String> = HashMap::new();
    let mut params_vec: Vec<crate::ast::Parameter> = Vec::new();
    let mut arg_metadata_by_name: HashMap<String, usize> = HashMap::new();

    // Process in reverse order (project overrides global)
    for statement in program.statements.iter().rev() {
        let (name, attributes, params) = match statement {
            Statement::SimpleFunctionDef {
                name,
                attributes,
                params,
                ..
            }
            | Statement::BlockFunctionDef {
                name,
                attributes,
                params,
                ..
            } => (name, attributes, params),
            _ => continue,
        };

        if name == tool_name {
            // Extract argument metadata from @arg attributes
            for attr in attributes {
                if let Attribute::Arg(arg_meta) = attr {
                    // Legacy mode: explicit position like @arg 1:name
                    if arg_meta.position > 0 {
                        arg_mapping.insert(arg_meta.position, arg_meta.name.clone());
                    } else {
                        // New mode: position 0 means match by name later
                        arg_metadata_by_name.insert(arg_meta.name.clone(), 0);
                    }
                }
            }
            params_vec = params.clone();
            break; // Take the first match (which is project due to reverse order)
        }
    }

    // Smart matching: match parameters with @arg by name, or use parameter order
    if !params_vec.is_empty() {
        for (idx, param) in params_vec.iter().enumerate() {
            if param.is_rest {
                continue;
            }

            let position = idx + 1;

            // Check if we already have this position from explicit @arg (legacy mode)
            if arg_mapping.contains_key(&position) {
                continue;
            }

            // Check if there's an @arg with matching name (new mode)
            if arg_metadata_by_name.contains_key(&param.name) {
                arg_mapping.insert(position, param.name.clone());
            } else if arg_mapping.values().any(|v| v == &param.name) {
                // Already mapped by legacy @arg with explicit position
                continue;
            } else {
                // No @arg metadata for this param, just use parameter order
                arg_mapping.insert(position, param.name.clone());
            }
        }
    }

    // Check for rest parameter — expand JSON array directly
    let rest_param = params_vec.iter().find(|p| p.is_rest);
    if let Some(rest) = rest_param
        && let Some(args_obj) = json_args.as_object()
        && let Some(serde_json::Value::Array(arr)) = args_obj.get(&rest.name)
    {
        let mut positional_args = Vec::new();
        // First add any non-rest positional args
        let max_position = *arg_mapping.keys().max().unwrap_or(&0);
        if max_position > 0 {
            positional_args.resize(max_position, String::new());
            for (position, param_name) in &arg_mapping {
                if let Some(value) = args_obj.get(param_name) {
                    let arg_str = value_to_string(value);
                    if *position > 0 && *position <= positional_args.len() {
                        positional_args[position - 1] = arg_str;
                    }
                }
            }
        }
        // Then append all rest args
        for item in arr {
            positional_args.push(value_to_string(item));
        }
        return Ok(positional_args);
    }

    // If still no mapping found, return empty arguments
    if arg_mapping.is_empty() {
        return Ok(Vec::new());
    }

    // Get the JSON object with arguments
    let args_obj = json_args.as_object().ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Arguments must be an object".to_string(),
        data: None,
    })?;

    // Build positional arguments array
    let max_position = *arg_mapping.keys().max().unwrap_or(&0);
    let mut positional_args = vec![String::new(); max_position];

    for (position, param_name) in &arg_mapping {
        if let Some(value) = args_obj.get(param_name) {
            let arg_str = value_to_string(value);

            if *position > 0 && *position <= positional_args.len() {
                positional_args[position - 1] = arg_str;
            }
        }
    }

    Ok(positional_args)
}

/// Convert a JSON value to a string for shell argument passing
fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_value_to_string_string() {
        assert_eq!(value_to_string(&json!("hello")), "hello");
    }

    #[test]
    fn test_value_to_string_number() {
        assert_eq!(value_to_string(&json!(42)), "42");
    }

    #[test]
    fn test_value_to_string_float() {
        assert_eq!(value_to_string(&json!(3.14)), "3.14");
    }

    #[test]
    fn test_value_to_string_bool_true() {
        assert_eq!(value_to_string(&json!(true)), "true");
    }

    #[test]
    fn test_value_to_string_bool_false() {
        assert_eq!(value_to_string(&json!(false)), "false");
    }

    #[test]
    fn test_value_to_string_null() {
        assert_eq!(value_to_string(&json!(null)), "");
    }

    #[test]
    fn test_value_to_string_array() {
        let val = json!([1, 2, 3]);
        let result = value_to_string(&val);
        assert_eq!(result, "[1,2,3]");
    }

    #[test]
    fn test_value_to_string_object() {
        let val = json!({"key": "value"});
        let result = value_to_string(&val);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }
}
