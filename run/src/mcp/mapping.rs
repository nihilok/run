//! Argument mapping and tool name resolution

#![allow(clippy::manual_let_else)]

use super::handlers::JsonRpcError;
use crate::ast::{Attribute, Program, Statement};
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
        message: format!(
            "Runfile syntax error: {}",
            parser::ParseError::from_pest(&e, &config_content, Some("Runfile"))
        ),
        data: None,
    })?;
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

fn load_merged_program() -> Result<Program, JsonRpcError> {
    let (config_content, _metadata) = config::load_merged_config().ok_or_else(|| JsonRpcError {
        code: -32603,
        message: "No Runfile found".to_string(),
        data: None,
    })?;

    parser::parse_script(&config_content).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!(
            "Runfile syntax error: {}",
            parser::ParseError::from_pest(&e, &config_content, Some("Runfile"))
        ),
        data: None,
    })
}

fn collect_arg_metadata(
    program: &Program,
    tool_name: &str,
) -> (
    HashMap<usize, String>,
    Vec<crate::ast::Parameter>,
    HashMap<String, usize>,
) {
    let mut arg_mapping: HashMap<usize, String> = HashMap::new();
    let mut params_vec: Vec<crate::ast::Parameter> = Vec::new();
    let mut arg_metadata_by_name: HashMap<String, usize> = HashMap::new();

    for statement in program.statements.iter().rev() {
        let (Statement::SimpleFunctionDef {
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
        }) = statement
        else {
            continue;
        };

        if name != tool_name {
            continue;
        }

        for attr in attributes {
            if let Attribute::Arg(arg_meta) = attr {
                if arg_meta.position > 0 {
                    arg_mapping.insert(arg_meta.position, arg_meta.name.clone());
                } else {
                    arg_metadata_by_name.insert(arg_meta.name.clone(), 0);
                }
            }
        }
        params_vec.clone_from(params);
        break;
    }

    (arg_mapping, params_vec, arg_metadata_by_name)
}

/// Map JSON arguments to positional shell arguments
/// Uses merged global+project config with project taking precedence
pub(super) fn map_arguments_to_positional(
    tool_name: &str,
    json_args: &serde_json::Value,
) -> Result<Vec<String>, JsonRpcError> {
    let program = load_merged_program()?;
    let (mut arg_mapping, params_vec, arg_metadata_by_name) =
        collect_arg_metadata(&program, tool_name);

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
            } else if !arg_mapping.values().any(|v| v == &param.name) {
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
    use serial_test::serial;
    use std::env;
    use std::fs;
    use tempfile::tempdir;

    /// Create a temp dir with a Runfile, return (`TempDir`, path).
    /// Caller must hold `TempDir` alive for the duration of the test.
    fn setup_runfile(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempdir().expect("Failed to create temp dir");
        fs::write(temp.path().join("Runfile"), content).expect("Failed to write Runfile");
        let path = temp.path().to_path_buf();
        (temp, path)
    }

    fn disable_global_merge() {
        // Safety: tests are run serially (#[serial]) so no concurrent env mutation
        unsafe { env::set_var("RUN_NO_GLOBAL_MERGE", "1") };
    }

    fn enable_global_merge() {
        // Safety: tests are run serially (#[serial]) so no concurrent env mutation
        unsafe { env::remove_var("RUN_NO_GLOBAL_MERGE") };
    }

    // ---- resolve_tool_name ----

    #[test]
    #[serial]
    fn test_resolve_tool_name_simple() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Deploy to environment\ndeploy(env: str) {\n    echo \"$env\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let result = resolve_tool_name("deploy");

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(result.unwrap(), "deploy");
    }

    #[test]
    #[serial]
    fn test_resolve_tool_name_colon_sanitised() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Deploy to staging\ndeploy:staging(region: str) {\n    echo \"$region\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let result = resolve_tool_name("deploy__staging");

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(result.unwrap(), "deploy:staging");
    }

    #[test]
    #[serial]
    fn test_resolve_tool_name_not_found() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile("# @desc Say hello\nhello() {\n    echo \"hello\"\n}\n");
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let result = resolve_tool_name("nonexistent_tool_xyz");

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Tool not found"));
    }

    #[test]
    #[serial]
    fn test_resolve_tool_name_no_runfile() {
        let original_cwd = env::current_dir().unwrap();
        let temp = tempdir().unwrap(); // no Runfile written
        env::set_current_dir(temp.path()).unwrap();
        disable_global_merge();

        let result = resolve_tool_name("____tool_that_cannot_exist____");

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        // Either no Runfile found (-32603) or tool not found (-32602) — both indicate failure
        let err = result.unwrap_err();
        assert!(
            err.code == -32603 || err.code == -32602,
            "Expected -32602 or -32603, got: {} - {}",
            err.code,
            err.message
        );
    }

    #[test]
    #[serial]
    fn test_resolve_tool_name_only_functions_with_desc_are_found() {
        let original_cwd = env::current_dir().unwrap();
        // "hidden" has no @desc, so it should NOT be resolvable as a tool
        let (_temp, path) =
            setup_runfile("# @desc Visible\nvisible() echo ok\nhidden() echo secret\n");
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let found = resolve_tool_name("visible");
        let not_found = resolve_tool_name("hidden");

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(found.unwrap(), "visible");
        assert_eq!(not_found.unwrap_err().code, -32602);
    }

    // ---- map_arguments_to_positional ----

    #[test]
    #[serial]
    fn test_map_arguments_named_params_in_order() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Deploy\ndeploy(env: str, version: str) {\n    echo \"$env $version\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let args = json!({ "env": "prod", "version": "2.0" });
        let result = map_arguments_to_positional("deploy", &args);

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let positional = result.unwrap();
        assert_eq!(positional[0], "prod");
        assert_eq!(positional[1], "2.0");
    }

    #[test]
    #[serial]
    fn test_map_arguments_rest_param_expands_array() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Run in container\ndocker_exec(container: str, ...command) {\n    echo \"$container\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let args = json!({ "container": "web", "command": ["ls", "-la", "/app"] });
        let result = map_arguments_to_positional("docker_exec", &args);

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(result.unwrap(), vec!["web", "ls", "-la", "/app"]);
    }

    #[test]
    #[serial]
    fn test_map_arguments_non_object_returns_error() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) =
            setup_runfile("# @desc Deploy\ndeploy(env: str) {\n    echo \"$env\"\n}\n");
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let args = json!("not an object");
        let result = map_arguments_to_positional("deploy", &args);

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Arguments must be an object"));
    }

    #[test]
    #[serial]
    fn test_map_arguments_legacy_arg_mapping() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Scale\n# @arg 1:service string Service\n# @arg 2:count int Count\nscale_service() {\n    echo \"$1 $2\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let args = json!({ "service": "web", "count": 3 });
        let result = map_arguments_to_positional("scale_service", &args);

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let positional = result.unwrap();
        assert_eq!(positional[0], "web");
        assert_eq!(positional[1], "3");
    }

    #[test]
    #[serial]
    fn test_map_arguments_no_params_returns_empty() {
        let original_cwd = env::current_dir().unwrap();
        // Function has no params and no @arg — arguments should be ignored
        let (_temp, path) = setup_runfile("# @desc Say hello\nhello() echo \"hello\"\n");
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let args = json!({});
        let result = map_arguments_to_positional("hello", &args);

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(result.unwrap(), Vec::<String>::new());
    }

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
        assert_eq!(value_to_string(&json!(1.23)), "1.23");
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
