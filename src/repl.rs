//! Interactive REPL (Read-Eval-Print Loop) for the run scripting language.

use crate::{config, parser, interpreter};
use std::env;
use std::io::{self, Write};

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Start an interactive shell (REPL) for the run scripting language.
pub fn run_repl() {
    let run_shell = env::var("RUN_SHELL").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            // Try to find pwsh (PowerShell 7+) first, then fallback to powershell (Windows PowerShell)
            if which::which("pwsh").is_ok() {
                "pwsh".to_string()
            } else {
                "powershell".to_string()
            }
        } else {
            "sh".to_string()
        }
    });
    println!("Run Shell {} ({})", PKG_VERSION, run_shell);
    println!("Type 'exit' or press Ctrl+D to quit\n");

    let mut interpreter = interpreter::Interpreter::new();

    // Load Runfile functions into the REPL
    if let Some(config_content) = config::load_config() {
        match parser::parse_script(&config_content) {
            Ok(program) => {
                if let Err(e) = interpreter.execute(program) {
                    eprintln!("Warning: Error loading Runfile functions: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Warning: Error parsing Runfile: {}", e);
            }
        }
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        // Print prompt
        print!("> ");
        stdout.flush().unwrap();

        // Read line
        let mut input = String::new();
        match stdin.read_line(&mut input) {
            Ok(0) => {
                // EOF (Ctrl+D)
                println!("\nGoodbye!");
                break;
            }
            Ok(_) => {
                let input = input.trim();

                // Check for exit command
                if input == "exit" || input == "quit" {
                    println!("Goodbye!");
                    break;
                }

                // Skip empty lines
                if input.is_empty() {
                    continue;
                }

                // Try to parse and execute the input
                match parser::parse_script(input) {
                    Ok(program) => {
                        if let Err(e) = interpreter.execute(program) {
                            eprintln!("Error: {}", e);
                        }
                    }
                    Err(e) => {
                        crate::executor::print_parse_error(&e, input, None);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }
}

