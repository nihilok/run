//! Tool schema definitions and inspection

use crate::ast::{Attribute, Statement};
use crate::{config, parser, utils};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON Schema for a tool parameter
#[derive(Debug, Serialize, Deserialize)]
pub struct ParameterSchema {
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
}

/// JSON Schema for tool input
#[derive(Debug, Serialize, Deserialize)]
pub struct InputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: HashMap<String, ParameterSchema>,
    pub required: Vec<String>,
}

/// A tool definition for MCP
#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: InputSchema,
}

pub const TOOL_SET_CWD: &str = "set_cwd";
pub const TOOL_GET_CWD: &str = "get_cwd";

/// Root structure for inspect output
#[derive(Debug, Serialize, Deserialize)]
pub struct InspectOutput {
    pub tools: Vec<Tool>,
}

/// Returns the built-in tools provided by the MCP server itself
#[must_use]
pub fn get_builtin_tools() -> Vec<Tool> {
    let mut tools = Vec::new();

    // set_cwd
    let mut set_cwd_props = HashMap::new();
    set_cwd_props.insert(
        "path".to_string(),
        ParameterSchema {
            param_type: "string".to_string(),
            description: "The path to switch to (relative or absolute)".to_string(),
        },
    );
    tools.push(Tool {
        name: TOOL_SET_CWD.to_string(),
        description: "Set the current working directory. Call this before other tools to change their execution context.".to_string(),
        input_schema: InputSchema {
            schema_type: "object".to_string(),
            properties: set_cwd_props,
            required: vec!["path".to_string()],
        },
    });

    // get_cwd
    tools.push(Tool {
        name: TOOL_GET_CWD.to_string(),
        description: "Get the current working directory.".to_string(),
        input_schema: InputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
        },
    });

    tools
}

/// Sanitise an arg name for use as a JSON schema property key.
///
/// - Strips a trailing `?` suffix (treating it as an "optional" marker).
/// - Validates the remaining name against `^[a-zA-Z0-9_.-]{1,64}$`.
/// - If invalid characters remain after stripping, replaces them with `_`
///   and emits a warning to stderr.
///
/// Returns `(sanitised_name, is_optional)`.
fn sanitise_property_key(name: &str) -> (String, bool) {
    // Check for `?` optional suffix
    let (stripped, is_optional) = if let Some(s) = name.strip_suffix('?') {
        (s, true)
    } else {
        (name, false)
    };

    // Validate against MCP property key regex: ^[a-zA-Z0-9_.-]{1,64}$
    let is_valid = !stripped.is_empty()
        && stripped.len() <= 64
        && stripped
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));

    if is_valid {
        (stripped.to_string(), is_optional)
    } else {
        // Sanitise: replace invalid chars with `_`, truncate to 64
        let sanitised: String = stripped
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .take(64)
            .collect();

        let sanitised = if sanitised.is_empty() {
            "_".to_string()
        } else {
            sanitised
        };

        eprintln!(
            "Warning: arg name {name:?} contains characters invalid for a JSON schema property key; using {sanitised:?} instead"
        );

        (sanitised, is_optional)
    }
}

/// Extract metadata from function attributes and parameters
/// Returns None if the function has no @desc attribute
pub(super) fn extract_function_metadata(
    name: &str,
    attributes: &[Attribute],
    params: &[crate::ast::Parameter],
) -> Option<Tool> {
    let mut description: Option<String> = None;
    let mut properties = HashMap::new();
    let mut required = Vec::new();

    // Build a map of @arg descriptions (sanitised_name -> description)
    // Strip `?` from names so hybrid-mode lookups match param names correctly.
    let mut arg_descriptions: HashMap<String, String> = HashMap::new();

    // Get description from attributes and collect @arg descriptions
    for attr in attributes {
        match attr {
            Attribute::Desc(desc) => {
                description = Some(desc.clone());
            }
            Attribute::Arg(arg_meta) => {
                // Strip `?` when keying descriptions so lookups by param.name work
                let (key, _) = sanitise_property_key(&arg_meta.name);
                arg_descriptions.insert(key, arg_meta.description.clone());
            }
            _ => {}
        }
    }

    // If we have params, use them (takes precedence over @arg for type/default)
    if params.is_empty() {
        // Fall back to @arg attributes for backward compatibility
        for attr in attributes {
            if let Attribute::Arg(arg_meta) = attr {
                let param_type = utils::arg_type_to_json_type(&arg_meta.arg_type);
                let (sanitised_name, is_optional) = sanitise_property_key(&arg_meta.name);

                properties.insert(
                    sanitised_name.clone(),
                    ParameterSchema {
                        param_type: param_type.to_string(),
                        description: arg_meta.description.clone(),
                    },
                );

                // `?` suffix means the arg is optional — do not add to required
                if !is_optional {
                    required.push(sanitised_name);
                }
            }
        }
    } else {
        for param in params {
            let param_description = arg_descriptions
                .get(&param.name)
                .cloned()
                .unwrap_or_default();

            if param.is_rest {
                // Rest parameter: array type, not required
                properties.insert(
                    param.name.clone(),
                    ParameterSchema {
                        param_type: "array".to_string(),
                        description: param_description,
                    },
                );
            } else {
                properties.insert(
                    param.name.clone(),
                    ParameterSchema {
                        param_type: utils::arg_type_to_json_type(&param.param_type).to_string(),
                        description: param_description,
                    },
                );

                // Only required if no default value and not rest
                if param.default_value.is_none() {
                    required.push(param.name.clone());
                }
            }
        }
    }

    // Only return a tool if it has a description
    description.map(|desc| {
        // Sanitise tool name: MCP spec requires [a-zA-Z0-9_-] only
        // Replace colons with double underscores
        let sanitised_name = name.replace(':', "__");

        Tool {
            name: sanitised_name,
            description: desc,
            input_schema: InputSchema {
                schema_type: "object".to_string(),
                properties,
                required,
            },
        }
    })
}

/// Generate inspection output from Runfile
///
/// Scans both global (~/.runfile) and project (./Runfile) for functions with
/// `@desc` attributes and generates MCP tool definitions from their metadata.
/// Project functions take precedence over global functions with the same name.
///
/// # Errors
///
/// Returns `Err` if:
/// - The Runfile cannot be parsed (syntax errors)
/// - The parser encounters an unexpected error
pub fn inspect() -> Result<InspectOutput, String> {
    let Some((config_content, _metadata)) = config::load_merged_config() else {
        // No Runfile = no tools
        return Ok(InspectOutput { tools: Vec::new() });
    };

    let program = parser::parse_script(&config_content).map_err(|e| format!("Parse error: {e}"))?;

    let mut tools = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    // Process statements in reverse order to give precedence to later definitions (project)
    // Since we concatenated global first, then project, project definitions appear later
    for statement in program.statements.iter().rev() {
        match statement {
            Statement::SimpleFunctionDef {
                name,
                params,
                attributes,
                ..
            }
            | Statement::BlockFunctionDef {
                name,
                params,
                attributes,
                ..
            } => {
                if utils::matches_current_platform(attributes)
                    && !seen_names.contains(name)
                    && let Some(tool) = extract_function_metadata(name, attributes, params)
                {
                    tools.push(tool);
                    seen_names.insert(name.clone());
                }
            }
            _ => {}
        }
    }

    // Reverse to restore original order (since we processed in reverse)
    tools.reverse();

    Ok(InspectOutput { tools })
}

/// Print inspection output as JSON
pub fn print_inspect() {
    match inspect() {
        Ok(output) => match serde_json::to_string_pretty(&output) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("Error serialising output: {e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitise_property_key_valid() {
        let (key, optional) = sanitise_property_key("repo");
        assert_eq!(key, "repo");
        assert!(!optional);
    }

    #[test]
    fn test_sanitise_property_key_question_mark_optional() {
        let (key, optional) = sanitise_property_key("repo?");
        assert_eq!(key, "repo");
        assert!(optional);
    }

    #[test]
    fn test_sanitise_property_key_invalid_chars_replaced() {
        let (key, optional) = sanitise_property_key("my arg");
        assert_eq!(key, "my_arg");
        assert!(!optional);
    }

    #[test]
    fn test_sanitise_property_key_invalid_chars_with_question_mark() {
        let (key, optional) = sanitise_property_key("my arg?");
        assert_eq!(key, "my_arg");
        assert!(optional);
    }

    #[test]
    fn test_sanitise_property_key_truncated_to_64() {
        let long_name = "a".repeat(70);
        let (key, optional) = sanitise_property_key(&long_name);
        assert_eq!(key.len(), 64);
        assert!(!optional);
    }

    #[test]
    fn test_sanitise_property_key_valid_chars() {
        // All valid chars: alphanumeric, underscore, dot, hyphen
        let (key, optional) = sanitise_property_key("my-arg.v2_0");
        assert_eq!(key, "my-arg.v2_0");
        assert!(!optional);
    }

    #[test]
    fn test_extract_function_metadata_optional_arg() {
        use crate::ast::{ArgMetadata, ArgType, Attribute};

        let attributes = vec![
            Attribute::Desc("Get repo info".to_string()),
            Attribute::Arg(ArgMetadata {
                position: 1,
                name: "repo?".to_string(),
                arg_type: ArgType::String,
                description: "Optional repo name".to_string(),
            }),
        ];

        let tool = extract_function_metadata("get_info", &attributes, &[]).unwrap();

        // Property key should have `?` stripped
        assert!(tool.input_schema.properties.contains_key("repo"));
        assert!(!tool.input_schema.properties.contains_key("repo?"));

        // Optional arg must not appear in required
        assert!(!tool.input_schema.required.contains(&"repo".to_string()));
        assert!(tool.input_schema.required.is_empty());
    }

    #[test]
    fn test_extract_function_metadata_mixed_optional_required() {
        use crate::ast::{ArgMetadata, ArgType, Attribute};

        let attributes = vec![
            Attribute::Desc("Clone a repo".to_string()),
            Attribute::Arg(ArgMetadata {
                position: 1,
                name: "url".to_string(),
                arg_type: ArgType::String,
                description: "Repository URL".to_string(),
            }),
            Attribute::Arg(ArgMetadata {
                position: 2,
                name: "branch?".to_string(),
                arg_type: ArgType::String,
                description: "Optional branch".to_string(),
            }),
        ];

        let tool = extract_function_metadata("clone", &attributes, &[]).unwrap();

        assert_eq!(tool.input_schema.properties.len(), 2);
        assert!(tool.input_schema.properties.contains_key("url"));
        assert!(tool.input_schema.properties.contains_key("branch"));
        assert!(!tool.input_schema.properties.contains_key("branch?"));

        // Only url should be required
        assert_eq!(tool.input_schema.required.len(), 1);
        assert!(tool.input_schema.required.contains(&"url".to_string()));
        assert!(!tool.input_schema.required.contains(&"branch".to_string()));
    }

    #[test]
    fn test_extract_function_metadata_optional_arg_description_in_hybrid_mode() {
        use crate::ast::{ArgMetadata, ArgType, Attribute, Parameter};

        // Hybrid: @arg with `?` suffix providing description for param "repo"
        let attributes = vec![
            Attribute::Desc("Get info".to_string()),
            Attribute::Arg(ArgMetadata {
                position: 0,
                name: "repo?".to_string(),
                arg_type: ArgType::String,
                description: "Optional repo name".to_string(),
            }),
        ];

        let params = vec![Parameter {
            name: "repo".to_string(),
            param_type: ArgType::String,
            default_value: None,
            is_rest: false,
        }];

        let tool = extract_function_metadata("get_info", &attributes, &params).unwrap();

        // Description should be picked up even with `?` in @arg name
        let repo_param = tool.input_schema.properties.get("repo").unwrap();
        assert_eq!(repo_param.description, "Optional repo name");
    }

    #[test]
    fn test_get_builtin_tools() {
        let tools = get_builtin_tools();
        assert_eq!(tools.len(), 2);

        let set_cwd = tools
            .iter()
            .find(|t| t.name == TOOL_SET_CWD)
            .expect("set_cwd tool should exist");
        assert_eq!(
            set_cwd.description,
            "Set the current working directory. Call this before other tools to change their execution context."
        );
        assert_eq!(set_cwd.input_schema.schema_type, "object");
        assert!(set_cwd.input_schema.properties.contains_key("path"));
        assert_eq!(set_cwd.input_schema.properties["path"].param_type, "string");
        assert_eq!(set_cwd.input_schema.required, vec!["path".to_string()]);

        let get_cwd = tools
            .iter()
            .find(|t| t.name == TOOL_GET_CWD)
            .expect("get_cwd tool should exist");
        assert_eq!(get_cwd.description, "Get the current working directory.");
        assert_eq!(get_cwd.input_schema.schema_type, "object");
        assert!(get_cwd.input_schema.properties.is_empty());
        assert!(get_cwd.input_schema.required.is_empty());
    }
}
