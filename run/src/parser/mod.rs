//! Parser implementation using pest
//!
//! This module provides parsing functionality for Run scripts,
//! converting text input into an Abstract Syntax Tree (AST).

mod attributes;
mod block;
mod preprocessing;
mod shebang;

use crate::ast::{Expression, Program, Statement};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct ScriptParser;

/// Parse a Run script into an Abstract Syntax Tree (AST)
///
/// # Errors
///
/// Returns `Err` if the input contains syntax errors that violate the grammar,
/// such as:
/// - Invalid function definition syntax
/// - Malformed attribute directives
/// - Unmatched braces or parentheses
/// - Invalid command syntax
pub fn parse_script(input: &str) -> Result<Program, Box<pest::error::Error<Rule>>> {
    let preprocessed = preprocessing::preprocess_escaped_newlines(input);
    let pairs = ScriptParser::parse(Rule::program, &preprocessed)?;
    let mut statements = Vec::new();

    for pair in pairs {
        match pair.as_rule() {
            Rule::program => {
                for inner_pair in pair.into_inner() {
                    if inner_pair.as_rule() == Rule::item {
                        // Item wraps the actual content
                        if let Some(content) = inner_pair.into_inner().next() {
                            match content.as_rule() {
                                Rule::comment => {
                                    // Skip comments - attributes are collected in parse_statement
                                }
                                _ => {
                                    if let Some(stmt) = parse_statement(content, &preprocessed) {
                                        statements.push(stmt);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Rule::EOI | _ => {}
        }
    }

    Ok(Program { statements })
}

fn parse_statement(pair: pest::iterators::Pair<Rule>, original_input: &str) -> Option<Statement> {
    match pair.as_rule() {
        Rule::assignment => {
            let mut inner = pair.into_inner();
            let name = inner.next()?.as_str().to_string();
            let value_str = inner.next()?.as_str().to_string();
            Some(Statement::Assignment {
                name,
                value: Expression::String(value_str),
            })
        }
        Rule::function_def => {
            let span = pair.as_span();
            let line_num = original_input[..span.start()].lines().count();
            let attributes = attributes::parse_attributes_from_lines(original_input, line_num);

            let mut inner = pair.into_inner();
            let name = inner.next()?.as_str().to_string();

            // Check if next element is param_list or body
            let (params, body_pair) = if let Some(next) = inner.next() {
                if next.as_rule() == Rule::param_list {
                    // Parse parameters
                    let params = parse_param_list(next);
                    let body = inner.next()?;
                    (params, body)
                } else {
                    // No params, this is the body
                    (Vec::new(), next)
                }
            } else {
                return None;
            };

            // Parse body (block or command)
            match body_pair.as_rule() {
                Rule::block => {
                    // Parse and dedent block content
                    let full_content = block::parse_block_content(body_pair.as_str());

                    // Split into commands based on shell type
                    let commands = block::split_block_commands(&full_content, &attributes);

                    // Parse shebang from the content
                    let shebang = shebang::parse_shebang(full_content.trim());

                    Some(Statement::BlockFunctionDef {
                        name,
                        params,
                        commands,
                        attributes,
                        shebang,
                    })
                }
                Rule::command => {
                    let command_template = parse_command(body_pair);
                    Some(Statement::SimpleFunctionDef {
                        name,
                        params,
                        command_template,
                        attributes,
                    })
                }
                _ => None,
            }
        }
        Rule::function_call => {
            let mut inner = pair.into_inner();
            let name = inner.next()?.as_str().to_string();
            let mut args = Vec::new();
            if let Some(arg_list_pair) = inner.next()
                && arg_list_pair.as_rule() == Rule::argument_list
            {
                for arg_pair in arg_list_pair.into_inner() {
                    if arg_pair.as_rule() == Rule::argument {
                        // Extract the actual argument value
                        let arg_value =
                            if let Some(inner_arg) = arg_pair.clone().into_inner().next() {
                                match inner_arg.as_rule() {
                                    Rule::quoted_string => {
                                        // Remove quotes from quoted strings
                                        inner_arg.as_str().trim_matches('"').to_string()
                                    }
                                    Rule::variable | Rule::argument_word => {
                                        inner_arg.as_str().to_string()
                                    }
                                    _ => inner_arg.as_str().to_string(),
                                }
                            } else {
                                arg_pair.as_str().to_string()
                            };
                        args.push(arg_value);
                    }
                }
            }
            Some(Statement::FunctionCall { name, args })
        }
        Rule::command => {
            let command = parse_command(pair);
            Some(Statement::Command { command })
        }
        _ => None,
    }
}

fn parse_command(pair: pest::iterators::Pair<Rule>) -> String {
    let mut result = String::new();
    let mut last_was_assignment_prefix = false;

    for part in pair.into_inner() {
        // command_part wraps the actual token, so we need to get the inner rule
        let actual_part = if part.as_rule() == Rule::command_part {
            part.into_inner().next()
        } else {
            Some(part)
        };

        let Some(part) = actual_part else {
            continue;
        };

        match part.as_rule() {
            Rule::quoted_string => {
                if !result.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                result.push('"');
                result.push_str(part.as_str().trim_matches('"'));
                result.push('"');
                last_was_assignment_prefix = false;
            }
            Rule::variable => {
                // Don't add space before variable if last token ended with =
                if !last_was_assignment_prefix && !result.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                result.push_str(part.as_str());
                last_was_assignment_prefix = false;
            }
            Rule::operator => {
                result.push(' ');
                result.push_str(part.as_str());
                result.push(' ');
                last_was_assignment_prefix = false;
            }
            Rule::word => {
                if !result.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                let word_str = part.as_str();
                result.push_str(word_str);
                // Check if this word ends with = (like --port=)
                last_was_assignment_prefix = word_str.ends_with('=');
            }
            _ => {
                if !result.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                result.push_str(part.as_str());
                last_was_assignment_prefix = false;
            }
        }
    }

    result.trim().to_string()
}

fn parse_param_list(pair: pest::iterators::Pair<Rule>) -> Vec<crate::ast::Parameter> {
    let mut params = Vec::new();

    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::params {
            for param_pair in inner.into_inner() {
                if let Some(param) = parse_param(param_pair) {
                    params.push(param);
                }
            }
        }
    }

    params
}

fn parse_param(pair: pest::iterators::Pair<Rule>) -> Option<crate::ast::Parameter> {
    let mut inner = pair.into_inner();
    let first = inner.next()?;

    // Check for rest parameter (...args)
    if first.as_rule() == Rule::rest_param {
        // rest_param contains param_identifier
        let name = first.into_inner().next()?.as_str().to_string();
        return Some(crate::ast::Parameter {
            name,
            param_type: crate::ast::ArgType::String,
            default_value: None,
            is_rest: true,
        });
    }

    // Regular parameter (first is regular_param)
    // regular_param contains: param_identifier, optional param_type_annotation, optional param_default
    let mut param_inner = first.into_inner();
    let name = param_inner.next()?.as_str().to_string(); // This is param_identifier
    let mut param_type = crate::ast::ArgType::String; // Default
    let mut default_value = None;

    // Check for type annotation and default value
    for next in param_inner {
        match next.as_rule() {
            Rule::param_type_annotation => {
                if let Some(type_pair) = next.into_inner().next() {
                    param_type = match type_pair.as_str() {
                        "int" | "integer" => crate::ast::ArgType::Integer,
                        "bool" | "boolean" => crate::ast::ArgType::Boolean,
                        _ => crate::ast::ArgType::String,
                    };
                }
            }
            Rule::param_default => {
                if let Some(default_pair) = next.into_inner().next() {
                    let val = default_pair.as_str().trim();
                    // Strip surrounding quotes if present
                    let val = if (val.starts_with('"') && val.ends_with('"'))
                        || (val.starts_with('\'') && val.ends_with('\''))
                    {
                        &val[1..val.len() - 1]
                    } else {
                        val
                    };
                    default_value = Some(val.to_string());
                }
            }
            _ => {}
        }
    }

    Some(crate::ast::Parameter {
        name,
        param_type,
        default_value,
        is_rest: false,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ast::{ArgType, Attribute};

    #[test]
    fn test_parse_command_with_variable_after_equals() {
        let input = "server() echo port=${1:-8080}";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name,
            command_template,
            attributes,
            ..
        } = &result.statements[0]
        {
            assert_eq!(name, "server");
            assert_eq!(
                command_template, "echo port=${1:-8080}",
                "Command template has unexpected spacing"
            );
            assert_eq!(attributes.len(), 0);
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_parse_desc_attribute() {
        let input = r"
# @desc Restarts the docker containers
restart() docker compose restart
";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "restart");
            assert_eq!(attributes.len(), 1);

            if let Attribute::Desc(desc) = &attributes[0] {
                assert_eq!(desc, "Restarts the docker containers");
            } else {
                panic!("Expected Desc attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_parse_arg_attribute_with_type() {
        let input = r"
# @arg 1:service string The name of the service
scale() docker compose scale $1
";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "scale");
            assert_eq!(attributes.len(), 1);

            if let Attribute::Arg(arg) = &attributes[0] {
                assert_eq!(arg.position, 1);
                assert_eq!(arg.name, "service");
                assert_eq!(arg.arg_type, ArgType::String);
                assert_eq!(arg.description, "The name of the service");
            } else {
                panic!("Expected Arg attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_parse_arg_attribute_integer_type() {
        let input = r"
# @arg 2:replicas integer The number of instances
scale() docker compose scale $1=$2
";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "scale");
            assert_eq!(attributes.len(), 1);

            if let Attribute::Arg(arg) = &attributes[0] {
                assert_eq!(arg.position, 2);
                assert_eq!(arg.name, "replicas");
                assert_eq!(arg.arg_type, ArgType::Integer);
                assert_eq!(arg.description, "The number of instances");
            } else {
                panic!("Expected Arg attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_parse_arg_attribute_boolean_type() {
        let input = r#"
# @arg 1:verbose boolean Enable verbose output
test() echo "Verbose: $1"
"#;
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "test");
            assert_eq!(attributes.len(), 1);

            if let Attribute::Arg(arg) = &attributes[0] {
                assert_eq!(arg.position, 1);
                assert_eq!(arg.name, "verbose");
                assert_eq!(arg.arg_type, ArgType::Boolean);
                assert_eq!(arg.description, "Enable verbose output");
            } else {
                panic!("Expected Arg attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_parse_multiple_attributes() {
        let input = r"
# @desc Scale a specific service
# @arg 1:service string The service name
# @arg 2:replicas integer The number of instances
scale() docker compose scale $1=$2
";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "scale");
            assert_eq!(attributes.len(), 3);

            // Check desc
            if let Attribute::Desc(desc) = &attributes[0] {
                assert_eq!(desc, "Scale a specific service");
            } else {
                panic!("Expected Desc attribute at position 0");
            }

            // Check first arg
            if let Attribute::Arg(arg) = &attributes[1] {
                assert_eq!(arg.position, 1);
                assert_eq!(arg.name, "service");
                assert_eq!(arg.arg_type, ArgType::String);
            } else {
                panic!("Expected Arg attribute at position 1");
            }

            // Check second arg
            if let Attribute::Arg(arg) = &attributes[2] {
                assert_eq!(arg.position, 2);
                assert_eq!(arg.name, "replicas");
                assert_eq!(arg.arg_type, ArgType::Integer);
            } else {
                panic!("Expected Arg attribute at position 2");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_parse_arg_without_explicit_type() {
        let input = r#"
# @arg 1:name Some description without type
greet() echo "Hello, $1"
"#;
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "greet");
            assert_eq!(attributes.len(), 1);

            if let Attribute::Arg(arg) = &attributes[0] {
                assert_eq!(arg.position, 1);
                assert_eq!(arg.name, "name");
                // Should default to string
                assert_eq!(arg.arg_type, ArgType::String);
                assert_eq!(arg.description, "Some description without type");
            } else {
                panic!("Expected Arg attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_strip_quotes_from_desc() {
        let input = r#"
# @desc "Open a shell in the specified Docker container"
docker_shell() docker compose exec bash
"#;
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "docker_shell");
            assert_eq!(attributes.len(), 1);

            if let Attribute::Desc(desc) = &attributes[0] {
                assert_eq!(desc, "Open a shell in the specified Docker container");
            } else {
                panic!("Expected Desc attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_strip_quotes_from_arg() {
        let input = r#"
# @arg 1:container "The name of the container"
shell() docker compose exec $1 bash
"#;
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "shell");
            assert_eq!(attributes.len(), 1);

            if let Attribute::Arg(arg) = &attributes[0] {
                assert_eq!(arg.position, 1);
                assert_eq!(arg.name, "container");
                assert_eq!(arg.arg_type, ArgType::String);
                assert_eq!(arg.description, "The name of the container");
            } else {
                panic!("Expected Arg attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_arg_without_position_hybrid_mode() {
        let input = r"
# @arg service The service to scale
# @arg replicas Number of instances
scale(service, replicas) docker compose scale $service=$replicas
";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef {
            name, attributes, ..
        } = &result.statements[0]
        {
            assert_eq!(name, "scale");
            assert_eq!(attributes.len(), 2);

            if let Attribute::Arg(arg) = &attributes[0] {
                assert_eq!(arg.name, "service");
                assert_eq!(arg.description, "The service to scale");
                assert_eq!(arg.position, 0); // Marker for hybrid mode
            } else {
                panic!("Expected Arg attribute");
            }

            if let Attribute::Arg(arg) = &attributes[1] {
                assert_eq!(arg.name, "replicas");
                assert_eq!(arg.description, "Number of instances");
            } else {
                panic!("Expected Arg attribute");
            }
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    // Tests for RFC004 function signature notation

    #[test]
    fn test_function_with_params() {
        let input = "deploy(env, version) echo $env $version";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "deploy");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "env");
            assert_eq!(params[0].param_type, crate::ast::ArgType::String);
            assert_eq!(params[0].default_value, None);
            assert!(!params[0].is_rest);
            assert_eq!(params[1].name, "version");
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_function_with_typed_params() {
        let input = "scale(service: str, replicas: int) echo $service $replicas";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "scale");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "service");
            assert_eq!(params[0].param_type, crate::ast::ArgType::String);
            assert_eq!(params[1].name, "replicas");
            assert_eq!(params[1].param_type, crate::ast::ArgType::Integer);
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_function_with_default_values() {
        let input = r#"deploy(env, version = "latest") echo $env $version"#;
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "deploy");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "env");
            assert_eq!(params[0].default_value, None);
            assert_eq!(params[1].name, "version");
            assert_eq!(params[1].default_value, Some("latest".to_string()));
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_function_with_rest_param() {
        let input = "echo_all(...args) echo $args";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "echo_all");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
            assert!(params[0].is_rest);
            assert_eq!(params[0].default_value, None);
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_function_with_mixed_params_and_rest() {
        let input = "docker_exec(container, ...command) docker exec $container $command";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "docker_exec");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "container");
            assert!(!params[0].is_rest);
            assert_eq!(params[1].name, "command");
            assert!(params[1].is_rest);
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_quoted_default_with_comma() {
        let input = r#"test(val = "a,b,c") echo $val"#;
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "test");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "val");
            assert_eq!(params[0].default_value, Some("a,b,c".to_string()));
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_single_quoted_default() {
        let input = "test(val = 'hello world') echo $val";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "test");
            assert_eq!(params[0].default_value, Some("hello world".to_string()));
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_unquoted_default() {
        let input = "test(port = 8080) echo $port";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "test");
            assert_eq!(params[0].default_value, Some("8080".to_string()));
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_empty_parens_still_works() {
        let input = "greet() echo hello";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "greet");
            assert_eq!(params.len(), 0);
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_function_keyword_with_params() {
        let input = "function deploy(env, version) echo $env $version";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "deploy");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "env");
            assert_eq!(params[1].name, "version");
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_block_function_with_params() {
        let input = r#"deploy(env, version) {
    echo "Deploying to $env"
    echo "Version: $version"
}"#;
        let result = parse_script(input).unwrap();

        if let Statement::BlockFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "deploy");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "env");
            assert_eq!(params[1].name, "version");
        } else {
            panic!("Expected BlockFunctionDef");
        }
    }

    #[test]
    fn test_all_param_types() {
        let input = "test(s: string, i: integer, b: boolean) echo $s $i $b";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "test");
            assert_eq!(params.len(), 3);
            assert_eq!(params[0].param_type, crate::ast::ArgType::String);
            assert_eq!(params[1].param_type, crate::ast::ArgType::Integer);
            assert_eq!(params[2].param_type, crate::ast::ArgType::Boolean);
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }

    #[test]
    fn test_short_type_names() {
        let input = "test(s: str, i: int, b: bool) echo $s $i $b";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, params, .. } = &result.statements[0] {
            assert_eq!(name, "test");
            assert_eq!(params.len(), 3);
            assert_eq!(params[0].param_type, crate::ast::ArgType::String);
            assert_eq!(params[1].param_type, crate::ast::ArgType::Integer);
            assert_eq!(params[2].param_type, crate::ast::ArgType::Boolean);
        } else {
            panic!("Expected SimpleFunctionDef");
        }
    }
}
