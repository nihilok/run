//! Argument mapping and tool name resolution

#![allow(clippy::manual_let_else)]

use super::handlers::JsonRpcError;
use super::tools::sanitise_property_key;
use crate::ast::{Attribute, Program, Statement};
use crate::{config, parser};
use std::collections::{HashMap, HashSet};

/// Resolve a sanitised tool name back to the original function name
/// This is needed because MCP requires [a-zA-Z0-9_-] but we support colons in function names
/// Uses merged global+project config to ensure all exposed tools are resolvable
pub(super) fn resolve_tool_name(sanitised_name: &str) -> Result<String, JsonRpcError> {
    let (config_content, metadata) = config::load_merged_config().ok_or_else(|| JsonRpcError {
        code: -32603,
        message: "No Runfile found".to_string(),
        data: None,
    })?;

    let program = parser::parse_script(&config_content).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!(
            "Runfile syntax error: {}",
            parser::ParseError::from_pest_merged(&e, &config_content, &metadata)
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
    let (config_content, metadata) = config::load_merged_config().ok_or_else(|| JsonRpcError {
        code: -32603,
        message: "No Runfile found".to_string(),
        data: None,
    })?;

    parser::parse_script(&config_content).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!(
            "Runfile syntax error: {}",
            parser::ParseError::from_pest_merged(&e, &config_content, &metadata)
        ),
        data: None,
    })
}

struct ToolArgSpec {
    /// Positional mapping: 1-based position -> sanitised MCP property key
    positional: HashMap<usize, String>,
    /// Rest parameter name (if signature defines `...rest`)
    rest_param: Option<String>,
    /// All declared MCP property keys (excluding built-in `timeout`)
    declared_keys: HashSet<String>,
    /// Required MCP property keys in declaration order
    required_keys: Vec<String>,
}

fn collect_arg_spec(program: &Program, tool_name: &str) -> ToolArgSpec {
    let mut positional: HashMap<usize, String> = HashMap::new();
    let mut rest_param: Option<String> = None;
    let mut declared_keys: HashSet<String> = HashSet::new();
    let mut required_keys: Vec<String> = Vec::new();

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

        if params.is_empty() {
            for attr in attributes {
                if let Attribute::Arg(arg_meta) = attr {
                    let (sanitised_name, is_optional) = sanitise_property_key(&arg_meta.name);
                    declared_keys.insert(sanitised_name.clone());
                    if !is_optional && !required_keys.contains(&sanitised_name) {
                        required_keys.push(sanitised_name.clone());
                    }
                    if arg_meta.position > 0 {
                        positional.insert(arg_meta.position, sanitised_name);
                    }
                }
            }
        } else {
            for (idx, param) in params.iter().enumerate() {
                declared_keys.insert(param.name.clone());
                if param.is_rest {
                    rest_param = Some(param.name.clone());
                } else {
                    positional.insert(idx + 1, param.name.clone());
                    if param.default_value.is_none() {
                        required_keys.push(param.name.clone());
                    }
                }
            }
        }
        break;
    }

    ToolArgSpec {
        positional,
        rest_param,
        declared_keys,
        required_keys,
    }
}

/// Map JSON arguments to positional shell arguments
/// Uses merged global+project config with project taking precedence
pub(super) fn map_arguments_to_positional(
    tool_name: &str,
    json_args: &serde_json::Value,
) -> Result<Vec<String>, JsonRpcError> {
    let args_obj = json_args.as_object().ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Arguments must be an object".to_string(),
        data: None,
    })?;

    let program = load_merged_program()?;
    let spec = collect_arg_spec(&program, tool_name);

    let mut unexpected_keys: Vec<&str> = args_obj
        .keys()
        .filter(|k| !spec.declared_keys.contains(k.as_str()))
        .map(String::as_str)
        .collect();
    unexpected_keys.sort_unstable();

    let missing_keys: Vec<&str> = spec
        .required_keys
        .iter()
        .filter(|k| {
            matches!(
                args_obj.get(k.as_str()),
                None | Some(serde_json::Value::Null)
            )
        })
        .map(String::as_str)
        .collect();

    if !missing_keys.is_empty() || !unexpected_keys.is_empty() {
        let missing_joined = missing_keys.join(", ");
        let unexpected_joined = unexpected_keys.join(", ");
        let message = match (missing_keys.is_empty(), unexpected_keys.is_empty()) {
            (false, false) => format!(
                "Missing required argument(s) for tool '{tool_name}': {missing_joined}; unexpected argument(s): {unexpected_joined}"
            ),
            (false, true) => {
                format!("Missing required argument(s) for tool '{tool_name}': {missing_joined}")
            }
            (true, false) => {
                format!("Unexpected argument(s) for tool '{tool_name}': {unexpected_joined}")
            }
            (true, true) => unreachable!(),
        };
        return Err(JsonRpcError {
            code: -32602,
            message,
            data: None,
        });
    }

    let rest_items = match spec.rest_param.as_deref() {
        Some(rest_name) => match args_obj.get(rest_name) {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Array(arr)) => {
                Some(arr.iter().map(value_to_string).collect::<Vec<_>>())
            }
            Some(other) => {
                return Err(JsonRpcError {
                    code: -32602,
                    message: format!(
                        "Invalid value for '{rest_name}': expected an array, got {other}"
                    ),
                    data: None,
                });
            }
        },
        None => None,
    };

    let positional_len = if spec.rest_param.is_some() {
        spec.positional.keys().copied().max().unwrap_or(0)
    } else {
        spec.positional
            .iter()
            .filter_map(|(&pos, key)| match args_obj.get(key) {
                Some(v) if !v.is_null() => Some(pos),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    };

    let mut positional_args = vec![String::new(); positional_len];
    for (position, param_name) in &spec.positional {
        if *position > 0
            && *position <= positional_args.len()
            && let Some(value) = args_obj.get(param_name)
            && !value.is_null()
        {
            positional_args[*position - 1] = value_to_string(value);
        }
    }

    if let Some(items) = rest_items {
        positional_args.extend(items);
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
        // Function has no params and no @arg — empty object produces empty positional args
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
    #[serial]
    fn test_map_arguments_missing_required_param_returns_error() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Deploy\ndeploy(env: str, region: str, version = \"latest\") {\n    echo \"$env $region $version\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let missing_one = map_arguments_to_positional("deploy", &json!({ "env": "prod" }));
        let missing_both = map_arguments_to_positional("deploy", &json!({}));

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let err_one = missing_one.unwrap_err();
        assert_eq!(err_one.code, -32602);
        assert!(err_one.message.contains("region"), "{}", err_one.message);

        let err_both = missing_both.unwrap_err();
        assert_eq!(err_both.code, -32602);
        assert!(err_both.message.contains("env"), "{}", err_both.message);
        assert!(err_both.message.contains("region"), "{}", err_both.message);
    }

    #[test]
    #[serial]
    fn test_map_arguments_null_required_param_returns_error() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) =
            setup_runfile("# @desc Deploy\ndeploy(env: str) {\n    echo \"$env\"\n}\n");
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let result = map_arguments_to_positional("deploy", &json!({ "env": null }));

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("env"), "{}", err.message);
    }

    #[test]
    #[serial]
    fn test_map_arguments_legacy_arg_missing_and_null_return_error() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Scale\n# @arg 1:service string Service\n# @arg 2:count int Count\nscale_service() {\n    echo \"$1 $2\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let missing = map_arguments_to_positional("scale_service", &json!({ "service": "web" }));
        let null_val = map_arguments_to_positional(
            "scale_service",
            &json!({ "service": "web", "count": null }),
        );

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let err_missing = missing.unwrap_err();
        assert_eq!(err_missing.code, -32602);
        assert!(
            err_missing.message.contains("count"),
            "{}",
            err_missing.message
        );

        let err_null = null_val.unwrap_err();
        assert_eq!(err_null.code, -32602);
        assert!(err_null.message.contains("count"), "{}", err_null.message);
    }

    #[test]
    #[serial]
    fn test_map_arguments_legacy_optional_question_mark_arg() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Clone\n# @arg 1:url string Repo URL\n# @arg 2:branch? string Optional branch\nclone() {\n    echo \"$1 $2\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let omitted =
            map_arguments_to_positional("clone", &json!({ "url": "https://example.com" }));
        let null_opt = map_arguments_to_positional(
            "clone",
            &json!({ "url": "https://example.com", "branch": null }),
        );
        let provided = map_arguments_to_positional(
            "clone",
            &json!({ "url": "https://example.com", "branch": "main" }),
        );
        let raw_question_key = map_arguments_to_positional(
            "clone",
            &json!({ "url": "https://example.com", "branch?": "main" }),
        );

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(omitted.unwrap(), vec!["https://example.com"]);
        assert_eq!(null_opt.unwrap(), vec!["https://example.com"]);
        assert_eq!(provided.unwrap(), vec!["https://example.com", "main"]);

        let err = raw_question_key.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("branch?"), "{}", err.message);
    }

    #[test]
    #[serial]
    fn test_map_arguments_legacy_sanitised_invalid_chars() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Greet\n# @arg 1:my$arg string Value\ngreet() {\n    echo \"$1\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let result = map_arguments_to_positional("greet", &json!({ "my_arg": "hello" }));

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(result.unwrap(), vec!["hello"]);
    }

    #[test]
    #[serial]
    fn test_map_arguments_optional_default_param_omitted_and_null() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Deploy\ndeploy(env: str, version = \"latest\") {\n    echo \"$env $version\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let omitted = map_arguments_to_positional("deploy", &json!({ "env": "prod" }));
        let null_opt =
            map_arguments_to_positional("deploy", &json!({ "env": "prod", "version": null }));
        let provided =
            map_arguments_to_positional("deploy", &json!({ "env": "prod", "version": "1.2.3" }));

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(omitted.unwrap(), vec!["prod"]);
        assert_eq!(null_opt.unwrap(), vec!["prod"]);
        assert_eq!(provided.unwrap(), vec!["prod", "1.2.3"]);
    }

    #[test]
    #[serial]
    fn test_map_arguments_rest_param_validation_and_optionality() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Run in container\ndocker_exec(container: str, ...command) {\n    echo \"$container\"\n}\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let omitted_rest =
            map_arguments_to_positional("docker_exec", &json!({ "container": "web" }));
        let null_rest = map_arguments_to_positional(
            "docker_exec",
            &json!({ "container": "web", "command": null }),
        );
        let empty_rest = map_arguments_to_positional(
            "docker_exec",
            &json!({ "container": "web", "command": [] }),
        );
        let missing_required_with_rest =
            map_arguments_to_positional("docker_exec", &json!({ "command": ["ls"] }));
        let unknown_with_rest = map_arguments_to_positional(
            "docker_exec",
            &json!({ "container": "web", "command": ["ls"], "bogus": 1 }),
        );
        let non_array_rest = map_arguments_to_positional(
            "docker_exec",
            &json!({ "container": "web", "command": "ls" }),
        );

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        assert_eq!(omitted_rest.unwrap(), vec!["web"]);
        assert_eq!(null_rest.unwrap(), vec!["web"]);
        assert_eq!(empty_rest.unwrap(), vec!["web"]);

        let err_missing = missing_required_with_rest.unwrap_err();
        assert_eq!(err_missing.code, -32602);
        assert!(
            err_missing.message.contains("container"),
            "{}",
            err_missing.message
        );

        let err_unknown = unknown_with_rest.unwrap_err();
        assert_eq!(err_unknown.code, -32602);
        assert!(
            err_unknown.message.contains("bogus"),
            "{}",
            err_unknown.message
        );

        let err_non_array = non_array_rest.unwrap_err();
        assert_eq!(err_non_array.code, -32602);
        assert!(
            err_non_array.message.contains("command"),
            "{}",
            err_non_array.message
        );
    }

    #[test]
    #[serial]
    fn test_map_arguments_unexpected_keys_rejected_for_all_tool_shapes() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile(
            "# @desc Zero arg\nping() echo pong\n\n# @desc Params\ngreet(name: str) echo \"$name\"\n\n# @desc Legacy\n# @arg 1:target string Target\nbuild() echo \"$1\"\n",
        );
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let zero_arg_err = map_arguments_to_positional("ping", &json!({ "extra": "x" }));
        let param_err =
            map_arguments_to_positional("greet", &json!({ "name": "world", "typo": 123 }));
        let legacy_err =
            map_arguments_to_positional("build", &json!({ "target": "app", "foo": true }));

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        let e1 = zero_arg_err.unwrap_err();
        assert_eq!(e1.code, -32602);
        assert!(e1.message.contains("extra"), "{}", e1.message);

        let e2 = param_err.unwrap_err();
        assert_eq!(e2.code, -32602);
        assert!(e2.message.contains("typo"), "{}", e2.message);

        let e3 = legacy_err.unwrap_err();
        assert_eq!(e3.code, -32602);
        assert!(e3.message.contains("foo"), "{}", e3.message);
    }

    #[test]
    #[serial]
    fn test_map_arguments_non_object_rejected_on_zero_arg_tool() {
        let original_cwd = env::current_dir().unwrap();
        let (_temp, path) = setup_runfile("# @desc Ping\nping() echo pong\n");
        env::set_current_dir(&path).unwrap();
        disable_global_merge();

        let results = [
            map_arguments_to_positional("ping", &json!("str")),
            map_arguments_to_positional("ping", &json!(null)),
            map_arguments_to_positional("ping", &json!([1])),
            map_arguments_to_positional("ping", &json!(42)),
        ];

        enable_global_merge();
        env::set_current_dir(original_cwd).unwrap();

        for result in results {
            let err = result.unwrap_err();
            assert_eq!(err.code, -32602);
            assert!(
                err.message.contains("Arguments must be an object"),
                "{}",
                err.message
            );
        }
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
