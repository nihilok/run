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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_tools() {
        let tools = get_builtin_tools();
        assert_eq!(tools.len(), 2);

        let set_cwd = tools.iter().find(|t| t.name == TOOL_SET_CWD).unwrap();
        assert_eq!(set_cwd.description, "Set the current working directory. Call this before other tools to change their execution context.");
        assert_eq!(set_cwd.input_schema.schema_type, "object");
        assert!(set_cwd.input_schema.properties.contains_key("path"));
        assert_eq!(set_cwd.input_schema.properties["path"].param_type, "string");
        assert_eq!(set_cwd.input_schema.required, vec!["path".to_string()]);

        let get_cwd = tools.iter().find(|t| t.name == TOOL_GET_CWD).unwrap();
        assert_eq!(get_cwd.description, "Get the current working directory.");
        assert_eq!(get_cwd.input_schema.schema_type, "object");
        assert!(get_cwd.input_schema.properties.is_empty());
        assert!(get_cwd.input_schema.required.is_empty());
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

    // Build a map of @arg descriptions (name -> description)
    let mut arg_descriptions: HashMap<String, String> = HashMap::new();

    // Get description from attributes and collect @arg descriptions
    for attr in attributes {
        match attr {
            Attribute::Desc(desc) => {
                description = Some(desc.clone());
            }
            Attribute::Arg(arg_meta) => {
                // Store description keyed by name for lookup
                arg_descriptions.insert(arg_meta.name.clone(), arg_meta.description.clone());
            }
            _ => {}
        }
    }

    // If we have params, use them (takes precedence over @arg for type/default)
    if !params.is_empty() {
        for param in params.iter() {
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
    } else {
        // Fall back to @arg attributes for backward compatibility
        for attr in attributes {
            if let Attribute::Arg(arg_meta) = attr {
                let param_type = utils::arg_type_to_json_type(&arg_meta.arg_type);

                properties.insert(
                    arg_meta.name.clone(),
                    ParameterSchema {
                        param_type: param_type.to_string(),
                        description: arg_meta.description.clone(),
                    },
                );

                required.push(arg_meta.name.clone());
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
    let config_content = match config::load_merged_config() {
        Some((content, _metadata)) => content,
        None => return Ok(InspectOutput { tools: Vec::new() }), // No Runfile = no tools
    };

    let program =
        parser::parse_script(&config_content).map_err(|e| format!("Parse error: {}", e))?;

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
            } => {
                if utils::matches_current_platform(attributes) && !seen_names.contains(name) {
                    if let Some(tool) = extract_function_metadata(name, attributes, params) {
                        tools.push(tool);
                        seen_names.insert(name.clone());
                    }
                }
            }
            Statement::BlockFunctionDef {
                name,
                params,
                attributes,
                ..
            } => {
                if utils::matches_current_platform(attributes) && !seen_names.contains(name) {
                    if let Some(tool) = extract_function_metadata(name, attributes, params) {
                        tools.push(tool);
                        seen_names.insert(name.clone());
                    }
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
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Error serialising output: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
