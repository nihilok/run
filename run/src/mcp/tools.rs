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

/// Root structure for inspect output
#[derive(Debug, Serialize, Deserialize)]
pub struct InspectOutput {
    pub tools: Vec<Tool>,
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
/// Scans the Runfile for functions with `@desc` attributes and generates
/// MCP tool definitions from their metadata.
///
/// # Errors
///
/// Returns `Err` if:
/// - The Runfile cannot be parsed (syntax errors)
/// - The parser encounters an unexpected error
pub fn inspect() -> Result<InspectOutput, String> {
    let config_content = match config::load_config() {
        Some(content) => content,
        None => return Ok(InspectOutput { tools: Vec::new() }), // No Runfile = no tools
    };

    let program =
        parser::parse_script(&config_content).map_err(|e| format!("Parse error: {}", e))?;

    let mut tools = Vec::new();

    for statement in program.statements {
        match statement {
            Statement::SimpleFunctionDef {
                name,
                params,
                attributes,
                ..
            } => {
                if utils::matches_current_platform(&attributes) {
                    if let Some(tool) = extract_function_metadata(&name, &attributes, &params) {
                        tools.push(tool);
                    }
                }
            }
            Statement::BlockFunctionDef {
                name,
                params,
                attributes,
                ..
            } => {
                if utils::matches_current_platform(&attributes) {
                    if let Some(tool) = extract_function_metadata(&name, &attributes, &params) {
                        tools.push(tool);
                    }
                }
            }
            _ => {}
        }
    }

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
