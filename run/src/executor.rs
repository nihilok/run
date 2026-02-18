//! Script execution and error formatting.

use crate::{cli::OutputFormatArg, config, interpreter, parser};
use std::fs;
use std::path::PathBuf;

/// Parse and execute a script file.
///
/// # Arguments
/// * `script` - The script source code to parse and execute.
/// * `filename` - Optional filename for better error messages.
pub fn execute_script(script: &str, filename: Option<&str>) {
    let program = match parser::parse_script(script) {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!("{}", parser::ParseError::from_pest(&e, script, filename));
            std::process::exit(1);
        }
    };

    let mut interpreter = interpreter::Interpreter::new();
    if let Err(e) = interpreter.execute(program) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// Execute a script file by path.
pub fn execute_file(path: &PathBuf) {
    let script = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("error: could not read '{}': {}", path.display(), e);
            std::process::exit(1);
        }
    };

    execute_script(&script, Some(&path.to_string_lossy()));
}

/// Load function definitions from config and call a function with arguments.
///
/// # Arguments
/// * `function_name` - The function to call (may be nested, e.g. "docker shell").
/// * `args` - Arguments to pass to the function.
/// * `output_format` - How to format the output.
pub fn run_function_call(function_name: &str, args: &[String], output_format: OutputFormatArg) {
    let Some((config_content, _metadata)) = config::load_merged_config() else {
        eprintln!("{}", config::NO_RUNFILE_ERROR);
        std::process::exit(1);
    };

    let mut interpreter = interpreter::Interpreter::new();
    interpreter.set_output_mode(output_format.mode());

    match parser::parse_script(&config_content) {
        Ok(program) => {
            if let Err(e) = interpreter.execute(program) {
                eprintln!("error: failed to load functions: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!(
                "{}",
                parser::ParseError::from_pest(&e, &config_content, Some("Runfile"))
            );
            std::process::exit(1);
        }
    }

    let exec_result = interpreter.call_function_without_parens(function_name, args);

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
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// List all available functions from the Runfile.
pub fn list_functions() {
    let Some((merged_content, metadata)) = config::load_merged_config() else {
        eprintln!("{}", config::NO_RUNFILE_ERROR);
        std::process::exit(1);
    };

    let disable_global_merge = std::env::var("RUN_NO_GLOBAL_MERGE").is_ok();
    if metadata.has_global && metadata.has_project && !disable_global_merge {
        list_functions_with_sources();
    } else {
        match parser::parse_script(&merged_content) {
            Ok(program) => {
                let mut interpreter = interpreter::Interpreter::new();
                if let Err(e) = interpreter.execute(program) {
                    eprintln!("error: failed to load functions: {e}");
                    std::process::exit(1);
                }

                let functions = interpreter.list_available_functions();
                if functions.is_empty() {
                    println!("No functions defined in Runfile.");
                    std::process::exit(0);
                } else {
                    let source_label = if let Some(custom_path) = config::get_custom_runfile_path()
                    {
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
                eprintln!(
                    "{}",
                    parser::ParseError::from_pest(&e, &merged_content, Some("Runfile"))
                );
                std::process::exit(1);
            }
        }
    }
}

/// List functions with source information when both global and project runfiles exist.
fn list_functions_with_sources() {
    use std::collections::HashSet;

    let global_functions = if let Some(global_content) = config::load_home_runfile() {
        match parser::parse_script(&global_content) {
            Ok(program) => {
                let mut interp = interpreter::Interpreter::new();
                if let Err(e) = interp.execute(program) {
                    eprintln!("error: failed to load global functions: {e}");
                    std::process::exit(1);
                }
                interp.list_available_functions()
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    parser::ParseError::from_pest(&e, &global_content, Some("~/.runfile"))
                );
                std::process::exit(1);
            }
        }
    } else {
        Vec::new()
    };

    let project_functions = if let Some(project_path) = config::find_project_runfile_path() {
        if let Ok(project_content) = fs::read_to_string(&project_path) {
            match parser::parse_script(&project_content) {
                Ok(program) => {
                    let mut interp = interpreter::Interpreter::new();
                    if let Err(e) = interp.execute(program) {
                        eprintln!("error: failed to load project functions: {e}");
                        std::process::exit(1);
                    }
                    interp.list_available_functions()
                }
                Err(e) => {
                    eprintln!(
                        "{}",
                        parser::ParseError::from_pest(&e, &project_content, Some("Runfile"))
                    );
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

    let has_any = !project_functions.is_empty() || !global_only.is_empty();

    if !has_any {
        println!("No functions defined in Runfile.");
        std::process::exit(0);
    }

    println!("Available functions:");

    if !project_functions.is_empty() {
        println!("\n  From ./Runfile:");
        for func in &project_functions {
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use crate::parser;

    // Use an unclosed quote as the test input: `"` cannot be in a `word` and
    // starts a `quoted_string` that never closes, guaranteeing a parse failure.
    const BAD_INPUT: &str = "\"unclosed string";

    /// Helper: trigger a real pest parse failure and convert it.
    fn make_parse_error(input: &str, filename: Option<&str>) -> parser::ParseError {
        use crate::parser::ScriptParser;
        use pest::Parser;
        let raw = ScriptParser::parse(crate::parser::Rule::program, input)
            .expect_err("expected a parse error for this input");
        parser::ParseError::from_pest(&raw, input, filename)
    }

    #[test]
    fn test_parse_error_display_has_location_arrow() {
        let err = make_parse_error(BAD_INPUT, Some("Runfile"));
        let s = err.to_string();
        assert!(s.contains("-->"), "missing location arrow in:\n{s}");
    }

    #[test]
    fn test_parse_error_display_has_source_line() {
        let err = make_parse_error(BAD_INPUT, Some("Runfile"));
        let s = err.to_string();
        assert!(
            s.contains("unclosed string"),
            "source line missing in:\n{s}"
        );
    }

    #[test]
    fn test_parse_error_display_has_caret() {
        let err = make_parse_error(BAD_INPUT, Some("Runfile"));
        let s = err.to_string();
        assert!(s.contains('^'), "caret missing in:\n{s}");
    }

    #[test]
    fn test_parse_error_no_raw_rule_names() {
        let err = make_parse_error(BAD_INPUT, None);
        assert!(
            !err.message.contains("Rule::"),
            "raw rule name in message: {}",
            err.message
        );
    }
}
