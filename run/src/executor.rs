//! Script execution and error formatting.

use crate::{cli::OutputFormatArg, config, interpreter, parser};
use std::fs;
use std::path::PathBuf;

struct LineInfo {
    line: usize,
    message: String,
}

/// Extract line number from pest error message.
fn extract_line_from_error(error_str: &str) -> Option<LineInfo> {
    // Pest errors often contain " --> line:col" or similar patterns
    // This is a simple heuristic parser
    if let Some(pos) = error_str.find(" --> ") {
        let rest = &error_str[pos + 5..];
        if let Some(colon_pos) = rest.find(':')
            && let Ok(line) = rest[..colon_pos].parse::<usize>()
        {
            return Some(LineInfo {
                line,
                message: error_str.to_string(),
            });
        }
    }
    None
}

/// Get a specific line from source code.
fn get_line(source: &str, line_num: usize) -> Option<String> {
    source
        .lines()
        .nth(line_num.saturating_sub(1))
        .map(|s| s.to_string())
}

/// Print a parse error with context from the source code.
pub fn print_parse_error(error: &dyn std::error::Error, source: &str, filename: Option<&str>) {
    let error_str = error.to_string();

    // Try to extract line information from pest error
    if let Some(line_info) = extract_line_from_error(&error_str) {
        let file_prefix = filename.map(|f| format!("{}:", f)).unwrap_or_default();
        eprintln!(
            "Parse error in {}line {}: {}",
            file_prefix, line_info.line, line_info.message
        );

        // Show the problematic line if we can extract it
        if let Some(line_content) = get_line(source, line_info.line) {
            eprintln!();
            eprintln!("  {} | {}", line_info.line, line_content);
            eprintln!(
                "  {} | {}",
                " ".repeat(line_info.line.to_string().len()),
                "^".repeat(line_content.trim().len().max(1))
            );
        }
    } else {
        eprintln!("Parse error: {}", error_str);
    }
}

/// Parse and execute a script file.
///
/// # Arguments
/// * `script` - The script source code to parse and execute.
/// * `filename` - Optional filename for better error messages.
pub fn execute_script(script: &str, filename: Option<String>) {
    // Parse the script
    let program = match parser::parse_script(script) {
        Ok(prog) => prog,
        Err(e) => {
            print_parse_error(&e, script, filename.as_deref());
            std::process::exit(1);
        }
    };

    // Execute the program
    let mut interpreter = interpreter::Interpreter::new();
    if let Err(e) = interpreter.execute(program) {
        eprintln!("Execution error: {}", e);
        std::process::exit(1);
    }
}

/// Execute a script file by path.
pub fn execute_file(path: &PathBuf) {
    let script = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path.display(), e);
            std::process::exit(1);
        }
    };

    execute_script(&script, Some(path.to_string_lossy().to_string()));
}

/// Load function definitions from config and call a function with arguments.
///
/// # Arguments
/// * `function_name` - The function to call (may be nested, e.g. "docker shell").
/// * `args` - Arguments to pass to the function.
/// * `output_format` - How to format the output.
pub fn run_function_call(
    function_name: &str,
    args: &[String],
    output_format: OutputFormatArg,
) {
    // Load the config file from ~/.runfile or ./Runfile
    let config_content = config::load_config_or_exit();

    // Parse the config to load function definitions
    let mut interpreter = interpreter::Interpreter::new();
    interpreter.set_output_mode(output_format.mode());

    match parser::parse_script(&config_content) {
        Ok(program) => {
            // Execute to load function definitions
            if let Err(e) = interpreter.execute(program) {
                eprintln!("Error loading functions: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            print_parse_error(&e, &config_content, Some("Runfile"));
            std::process::exit(1);
        }
    }

    // Now execute the function call with arguments
    // For nested commands, try different combinations:
    // e.g., "docker shell app" -> try "docker:shell" with arg "app"
    if let Err(e) = interpreter.call_function_without_parens(function_name, args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // If in structured mode, output the captured results
    if matches!(output_format.mode(), crate::ast::OutputMode::Structured) {
        let outputs = interpreter.take_captured_outputs();
        if !outputs.is_empty() {
            let interpreter_name = interpreter.last_interpreter();

            let result = crate::ast::StructuredResult::from_outputs(
                function_name,
                outputs,
                interpreter_name,
            );

            if let Some(formatted) = output_format.format_result(&result) {
                println!("{}", formatted);
            }
        }
    }
}

/// List all available functions from the Runfile.
pub fn list_functions() {
    let config_content = config::load_config_or_exit();

    // Parse the config to extract function names
    match parser::parse_script(&config_content) {
        Ok(program) => {
            // Use interpreter to handle platform filtering
            let mut interpreter = interpreter::Interpreter::new();
            
            // Execute to load function definitions (this applies platform filtering)
            if let Err(e) = interpreter.execute(program) {
                eprintln!("Error loading functions: {}", e);
                std::process::exit(1);
            }
            
            // Get the list of available functions
            let functions = interpreter.list_available_functions();

            if functions.is_empty() {
                println!("No functions defined in Runfile.");
                // Exit with success since the file was found and parsed correctly
                std::process::exit(0);
            } else {
                println!("Available functions:");
                for func in functions {
                    println!("  {}", func);
                }
            }
        }
        Err(e) => {
            eprintln!("Error parsing Runfile: {}", e);
            std::process::exit(1);
        }
    }
}

