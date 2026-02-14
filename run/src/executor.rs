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
        .map(std::string::ToString::to_string)
}

/// Print a parse error with context from the source code.
pub fn print_parse_error(error: &dyn std::error::Error, source: &str, filename: Option<&str>) {
    let error_str = error.to_string();

    // Try to extract line information from pest error
    if let Some(line_info) = extract_line_from_error(&error_str) {
        let file_prefix = filename.map(|f| format!("{f}:")).unwrap_or_default();
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
        eprintln!("Parse error: {error_str}");
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
        eprintln!("Execution error: {e}");
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
pub fn run_function_call(function_name: &str, args: &[String], output_format: OutputFormatArg) {
    // Load and merge config files from both ~/.runfile and ./Runfile
    let config_content = if let Some((content, _metadata)) = config::load_merged_config() {
        content
    } else {
        eprintln!("{}", config::NO_RUNFILE_ERROR);
        std::process::exit(1);
    };

    // Parse the config to load function definitions
    let mut interpreter = interpreter::Interpreter::new();
    interpreter.set_output_mode(output_format.mode());

    match parser::parse_script(&config_content) {
        Ok(program) => {
            // Execute to load function definitions
            if let Err(e) = interpreter.execute(program) {
                eprintln!("Error loading functions: {e}");
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
    let exec_result = interpreter.call_function_without_parens(function_name, args);

    // If in structured mode, output the captured results (even on error, so stderr is not lost)
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
                println!("{formatted}");
            }
        }
    }

    if let Err(e) = exec_result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// List all available functions from the Runfile.
pub fn list_functions() {
    // Try to load both global and project runfiles
    let merge_result = config::load_merged_config();

    if merge_result.is_none() {
        eprintln!("{}", config::NO_RUNFILE_ERROR);
        std::process::exit(1);
    }

    // SAFETY: We just checked that merge_result is Some above
    let (merged_content, metadata) = match merge_result {
        Some(result) => result,
        None => unreachable!("merge_result is Some after is_none check"),
    };

    // If we have both files, parse them separately to show sources
    // (unless RUN_NO_GLOBAL_MERGE is set, in which case we already have only one file)
    let disable_global_merge = std::env::var("RUN_NO_GLOBAL_MERGE").is_ok();
    if metadata.has_global && metadata.has_project && !disable_global_merge {
        list_functions_with_sources();
    } else {
        // Single source, use simple listing
        match parser::parse_script(&merged_content) {
            Ok(program) => {
                let mut interpreter = interpreter::Interpreter::new();
                if let Err(e) = interpreter.execute(program) {
                    eprintln!("Error loading functions: {e}");
                    std::process::exit(1);
                }

                let functions = interpreter.list_available_functions();
                if functions.is_empty() {
                    println!("No functions defined in Runfile.");
                    std::process::exit(0);
                } else {
                    // Determine source label
                    let source_label = if let Some(custom_path) = config::get_custom_runfile_path()
                    {
                        // Custom runfile specified via --runfile
                        custom_path.display().to_string()
                    } else if metadata.has_global {
                        "~/.runfile".to_string()
                    } else {
                        "./Runfile".to_string()
                    };
                    println!("Available functions from {source_label}:");
                    for func in functions {
                        println!("  {func}");
                    }
                }
            }
            Err(e) => {
                eprintln!("Error parsing Runfile: {e}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_line_from_error_with_arrow() {
        let error = "some error --> 42:10 more text";
        let result = extract_line_from_error(error);
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.line, 42);
    }

    #[test]
    fn test_extract_line_from_error_no_arrow() {
        let error = "just a plain error message";
        let result = extract_line_from_error(error);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_line_from_error_invalid_line() {
        let error = "error --> abc:10";
        let result = extract_line_from_error(error);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_line_valid() {
        let source = "line one\nline two\nline three";
        assert_eq!(get_line(source, 1), Some("line one".to_string()));
        assert_eq!(get_line(source, 2), Some("line two".to_string()));
        assert_eq!(get_line(source, 3), Some("line three".to_string()));
    }

    #[test]
    fn test_get_line_out_of_bounds() {
        let source = "line one\nline two";
        assert_eq!(get_line(source, 5), None);
    }

    #[test]
    fn test_get_line_zero() {
        let source = "line one\nline two";
        // saturating_sub(1) on 0 gives 0, so nth(0) should return first line
        assert_eq!(get_line(source, 0), Some("line one".to_string()));
    }
}

/// List functions with source information when both global and project runfiles exist.
fn list_functions_with_sources() {
    use std::collections::HashSet;

    // Load and parse global runfile
    let global_functions = if let Some(global_content) = config::load_home_runfile() {
        match parser::parse_script(&global_content) {
            Ok(program) => {
                let mut interp = interpreter::Interpreter::new();
                if let Err(e) = interp.execute(program) {
                    eprintln!("Error loading global functions: {e}");
                    std::process::exit(1);
                }
                interp.list_available_functions()
            }
            Err(e) => {
                eprintln!("Error parsing ~/.runfile: {e}");
                std::process::exit(1);
            }
        }
    } else {
        Vec::new()
    };

    // Load and parse project runfile
    let project_functions = if let Some(project_path) = config::find_project_runfile_path() {
        if let Ok(project_content) = fs::read_to_string(&project_path) {
            match parser::parse_script(&project_content) {
                Ok(program) => {
                    let mut interp = interpreter::Interpreter::new();
                    if let Err(e) = interp.execute(program) {
                        eprintln!("Error loading project functions: {e}");
                        std::process::exit(1);
                    }
                    interp.list_available_functions()
                }
                Err(e) => {
                    eprintln!("Error parsing project Runfile: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let project_set: HashSet<_> = project_functions.iter().cloned().collect();
    let global_only: Vec<_> = global_functions
        .iter()
        .filter(|f| !project_set.contains(*f))
        .cloned()
        .collect();

    // Display results
    let has_any = !project_functions.is_empty() || !global_only.is_empty();

    if !has_any {
        println!("No functions defined in Runfile.");
        std::process::exit(0);
    }

    println!("Available functions:");

    if !project_functions.is_empty() {
        println!("\n  From ./Runfile:");
        for func in &project_functions {
            // Check if this overrides a global function
            if global_functions.contains(func) {
                println!("    {func} (overrides global)");
            } else {
                println!("    {func}");
            }
        }
    }

    if !global_only.is_empty() {
        println!("\n  From ~/.runfile:");
        for func in &global_only {
            println!("    {func}");
        }
    }
}
