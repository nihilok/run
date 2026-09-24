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

    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));
    let processed = config::expand_source_directives(&script, base_dir);

    let program = match parser::parse_script(&processed) {
        Ok(prog) => prog,
        Err(e) => {
            eprintln!(
                "{}",
                parser::ParseError::from_pest(&e, &processed, Some(&path.to_string_lossy()))
            );
            std::process::exit(1);
        }
    };

    let mut interpreter = interpreter::Interpreter::new();
    interpreter.set_runfile_dir(Some(base_dir.to_path_buf()));
    if let Err(e) = interpreter.execute(program) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
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
    show_script: bool,
    parallel: bool,
    jobs: Option<usize>,
) {
    let Some((config_content, _metadata)) = config::load_merged_config() else {
        eprintln!("{}", config::NO_RUNFILE_ERROR);
        std::process::exit(1);
    };

    let mut interpreter = interpreter::Interpreter::new();
    interpreter.set_output_mode(output_format.mode());
    interpreter.set_show_script(show_script);

    // Inject __RUNFILE_DIR__ from the resolved Runfile path.
    // Prefer the RUN_RUNFILE_DIR env var (set by the MCP handler when the subprocess is
    // given a temp merged file) so that __RUNFILE_DIR__ always points to the actual project
    // root, not the system temp directory where the merged file was written.
    // Only accept the env-var value when it points to an existing directory, so a stale or
    // malformed env var does not silently override a correctly resolved path.
    let runfile_dir = std::env::var_os("RUN_RUNFILE_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| config::find_runfile_path().and_then(|p| p.parent().map(PathBuf::from)));
    interpreter.set_runfile_dir(runfile_dir);

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

    config::set_mcp_function_name(function_name);

    // Resolve dependencies
    let resolved_target = interpreter.resolve_target(function_name, args);
    let mut exec_result = Ok(());

    if let Some((target_name, _target_args)) = &resolved_target {
        exec_result =
            execute_dependencies(&mut interpreter, target_name, parallel, jobs, show_script);
    }

    // Run the main target if dependencies succeeded
    if exec_result.is_ok() {
        exec_result = interpreter.call_function_without_parens(function_name, args);
    }

    if matches!(output_format.mode(), crate::ast::OutputMode::Structured) {
        let outputs = interpreter.take_captured_outputs();
        if !outputs.is_empty() {
            let interpreter_name = interpreter.last_interpreter();

            let result = crate::ast::StructuredResult::from_outputs_with_workdir(
                function_name,
                outputs,
                interpreter_name,
                interpreter.last_working_directory().map(String::from),
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

#[allow(clippy::too_many_lines)]
fn execute_dependencies(
    interpreter: &mut interpreter::Interpreter,
    target_name: &str,
    parallel: bool,
    jobs: Option<usize>,
    show_script: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let is_parallel = !show_script
        && (parallel || interpreter.is_parallel(target_name) || jobs.is_some_and(|j| j > 1))
        && jobs.is_none_or(|j| j > 1);

    if is_parallel {
        let stages = match crate::graph::resolve_dependency_stages(target_name, |name| {
            interpreter.get_dependencies(name)
        }) {
            Ok(stages) => stages,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        };

        if stages.len() <= 1 {
            return Ok(());
        }

        let max_jobs = jobs.filter(|&j| j > 0).unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get)
        });

        for stage in &stages[..stages.len() - 1] {
            if stage.is_empty() {
                continue;
            }

            if stage.len() == 1 {
                interpreter.call_function_without_parens(&stage[0], &[])?;
                continue;
            }

            let num_workers = stage.len().min(max_jobs);
            let task_queue = std::sync::Mutex::new(std::collections::VecDeque::from(stage.clone()));
            let aborted = std::sync::atomic::AtomicBool::new(false);
            let first_error: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
            let results_by_task: std::sync::Mutex<
                std::collections::HashMap<String, Vec<crate::ast::CommandOutput>>,
            > = std::sync::Mutex::new(std::collections::HashMap::new());
            let print_lock = std::sync::Mutex::new(());
            let original_mode = interpreter.output_mode();

            std::thread::scope(|s| {
                for _ in 0..num_workers {
                    s.spawn(|| {
                        let mut worker_interp = interpreter.clone();
                        worker_interp.set_output_mode(crate::ast::OutputMode::Structured);

                        loop {
                            if aborted.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }

                            let next_task = {
                                let Ok(mut q) = task_queue.lock() else {
                                    break;
                                };
                                q.pop_front()
                            };

                            let Some(task_name) = next_task else {
                                break;
                            };

                            let res = worker_interp.call_function_without_parens(&task_name, &[]);
                            let outputs = worker_interp.take_captured_outputs();

                            if matches!(
                                original_mode,
                                crate::ast::OutputMode::Stream | crate::ast::OutputMode::Capture
                            ) && let Ok(_lock) = print_lock.lock()
                            {
                                for out in &outputs {
                                    if !out.stdout.is_empty() {
                                        print!("{}", out.stdout);
                                    }
                                    if !out.stderr.is_empty() {
                                        eprint!("{}", out.stderr);
                                    }
                                }
                                let _ = std::io::stdout().flush();
                                let _ = std::io::stderr().flush();
                            }

                            if let Ok(mut results) = results_by_task.lock() {
                                results.insert(task_name.clone(), outputs);
                            }

                            if let Err(e) = res {
                                aborted.store(true, std::sync::atomic::Ordering::Relaxed);
                                if let Ok(mut err_slot) = first_error.lock()
                                    && err_slot.is_none()
                                {
                                    *err_slot = Some(e.to_string());
                                }
                                break;
                            }
                        }
                    });
                }
            });

            // Preserve deterministic stage task order for structured output
            if let Ok(mut results_map) = results_by_task.into_inner() {
                for task_name in stage {
                    if let Some(outputs) = results_map.remove(task_name) {
                        interpreter.extend_captured_outputs(outputs);
                    }
                }
            }

            if let Ok(Some(err_msg)) = first_error.into_inner() {
                return Err(err_msg.into());
            }
        }
    } else {
        let execution_order = match crate::graph::resolve_dependencies(target_name, |name| {
            interpreter.get_dependencies(name)
        }) {
            Ok(order) => order,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        };

        if execution_order.len() > 1 {
            for dep in &execution_order[..execution_order.len() - 1] {
                interpreter.call_function_without_parens(dep, &[])?;
            }
        }
    }

    Ok(())
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
