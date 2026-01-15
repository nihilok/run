// Parser implementation using pest

use crate::ast::{Attribute, Expression, OsPlatform, Program, ShellType, Statement};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct ScriptParser;

// Preprocess input to join lines ending with a backslash
fn preprocess_escaped_newlines(input: &str) -> String {
    let mut result = String::new();
    let lines = input.lines();
    let mut buffer = String::new();
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.ends_with('\\') {
            if let Some(stripped) = trimmed.strip_suffix('\\') {
                buffer.push_str(stripped);
            }
            buffer.push(' ');
        } else {
            buffer.push_str(trimmed);
            result.push_str(buffer.trim_end());
            result.push('\n');
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        result.push_str(buffer.trim_end());
        result.push('\n');
    }
    result
}

// Parse attributes from lines of the original input
fn parse_attributes_from_lines(input: &str, line_num: usize) -> Vec<Attribute> {
    let mut attributes = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    
    if line_num == 0 {
        return attributes;
    }
    
    // Look backward from the function definition line to collect attributes
    let mut i = line_num - 1;
    loop {
        // Check if index is valid
        if i >= lines.len() {
            break;
        }
        
        let line = lines[i].trim();
        
        // If we hit an empty line or a non-comment line, stop
        if line.is_empty() || (!line.starts_with('#')) {
            break;
        }
        
        // If it's an attribute comment, parse it
        if line.starts_with("# @") || line.starts_with("#@") {
            if let Some(attr) = parse_attribute_line(line) {
                attributes.push(attr);
            }
        } else if line.starts_with('#') {
            // Regular comment - continue looking backward
        } else {
            break;
        }
        
        if i == 0 {
            break;
        }
        i -= 1;
    }
    
    // Reverse since we collected them backward
    attributes.reverse();
    attributes
}

fn parse_attribute_line(line: &str) -> Option<Attribute> {
    // Parse "# @os <platform>" or "# @shell <shell>"
    let line = line.trim();
    
    // Remove "# " or "#" prefix and "@" symbol
    let without_hash = line.strip_prefix("# @").or_else(|| line.strip_prefix("#@"))?;
    let parts: Vec<&str> = without_hash.split_whitespace().collect();
    
    if parts.len() < 2 {
        return None;
    }
    
    match parts[0] {
        "os" => {
            let platform = match parts[1] {
                "windows" => OsPlatform::Windows,
                "linux" => OsPlatform::Linux,
                "macos" => OsPlatform::MacOS,
                "unix" => OsPlatform::Unix,
                _ => return None,
            };
            Some(Attribute::Os(platform))
        }
        "shell" => {
            let shell = match parts[1] {
                "python" => ShellType::Python,
                "python3" => ShellType::Python3,
                "node" => ShellType::Node,
                "ruby" => ShellType::Ruby,
                "pwsh" => ShellType::Pwsh,
                "bash" => ShellType::Bash,
                "sh" => ShellType::Sh,
                _ => return None,
            };
            Some(Attribute::Shell(shell))
        }
        _ => None,
    }
}

// Parse shebang from function body
// Returns the shebang string if found on the first non-empty, non-comment line
// Lines starting with # (but not #!) are skipped
fn parse_shebang(body: &str) -> Option<String> {
    body.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .find(|line| {
            // Find the first line that is either a shebang or not a comment
            !line.starts_with('#') || line.starts_with("#!")
        })
        .and_then(|line| {
            if line.starts_with("#!") {
                Some(line[2..].trim().to_string())
            } else {
                None
            }
        })
}

pub fn parse_script(input: &str) -> Result<Program, Box<pest::error::Error<Rule>>> {
    let preprocessed = preprocess_escaped_newlines(input);
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
            let attributes = parse_attributes_from_lines(original_input, line_num);
            
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
                        let shebang = parse_shebang(&trimmed_content);

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
mod tests {
    use super::*;

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
}
