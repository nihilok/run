//! Parser implementation using pest
//!
//! This module provides parsing functionality for Run scripts,
//! converting text input into an Abstract Syntax Tree (AST).

mod attributes;
mod preprocessing;
mod shebang;

use crate::ast::{Attribute, Expression, Program, Statement};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct ScriptParser;

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
                                    if let Some(stmt) = parse_statement(content, input) {
                                        statements.push(stmt);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Rule::EOI => {}
            _ => {}
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

            // The next element is either a command or a block
            if let Some(body_pair) = inner.next() {
                match body_pair.as_rule() {
                    Rule::block => {
                        // Get the block content by stripping the braces from the block
                        // block_content is an atomic rule, so we extract it from the raw string
                        let block_str = body_pair.as_str();
                        // Remove leading '{' and trailing '}' but DON'T trim - we need to preserve
                        // internal indentation structure for proper dedentation
                        let content_str = block_str
                            .strip_prefix('{')
                            .unwrap_or(block_str)
                            .strip_suffix('}')
                            .unwrap_or(block_str);

                        // Split by newlines to process line by line
                        let all_lines: Vec<&str> = content_str.lines().collect();

                        // Skip leading and trailing empty/whitespace-only lines
                        let start = all_lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
                        let end = all_lines.iter().rposition(|l| !l.trim().is_empty()).map(|i| i + 1).unwrap_or(all_lines.len());
                        let lines: Vec<&str> = if start < end { all_lines[start..end].to_vec() } else { vec![] };

                        // Find the minimum indentation (excluding empty lines)
                        let min_indent = lines.iter()
                            .filter(|line| !line.trim().is_empty())
                            .map(|line| {
                                let trimmed_start = line.len() - line.trim_start().len();
                                trimmed_start
                            })
                            .min()
                            .unwrap_or(0);

                        // Build dedented lines
                        let dedented_lines: Vec<String> = lines.iter()
                            .map(|line| {
                                if line.trim().is_empty() {
                                    String::new()
                                } else if line.len() > min_indent {
                                    line[min_indent..].to_string()
                                } else {
                                    line.to_string()
                                }
                            })
                            .collect();

                        // Join into a single command or split by semicolons for inline blocks
                        let full_content = dedented_lines.join("\n");

                        // Check if this function has a custom shell attribute
                        let has_custom_shell = attributes
                            .iter()
                            .any(|attr| matches!(attr, Attribute::Shell(_)));

                        let trimmed_content = full_content.trim().to_string();

                        let commands: Vec<String> = if has_custom_shell {
                            // For custom shells (Python, Node, etc.), never split by semicolons
                            // The entire script should be passed as-is to the interpreter
                            vec![trimmed_content.clone()]
                        } else if !trimmed_content.contains('\n') && trimmed_content.contains(';') {
                            // Single-line block with semicolons: split into separate commands
                            // e.g., { echo "a"; echo "b"; echo "c" }
                            trimmed_content
                                .split(';')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect()
                        } else {
                            // Multi-line block - keep as single script
                            vec![trimmed_content.clone()]
                        };

                        // Parse shebang from the content
                        let shebang = shebang::parse_shebang(&trimmed_content);

                        Some(Statement::BlockFunctionDef {
                            name,
                            commands,
                            attributes,
                            shebang,
                        })
                    }
                    Rule::command => {
                        let command_template = parse_command(body_pair);
                        Some(Statement::SimpleFunctionDef {
                            name,
                            command_template,
                            attributes,
                        })
                    }
                    _ => None,
                }
            } else {
                None
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ast::ArgType;

    #[test]
    fn test_parse_command_with_variable_after_equals() {
        let input = "server() echo port=${1:-8080}";
        let result = parse_script(input).unwrap();

        if let Statement::SimpleFunctionDef { name, command_template, attributes } = &result.statements[0] {
            assert_eq!(name, "server");
            assert_eq!(command_template, "echo port=${1:-8080}", "Command template has unexpected spacing");
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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

        if let Statement::SimpleFunctionDef { name, attributes, .. } = &result.statements[0] {
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
}
