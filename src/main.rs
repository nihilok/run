//! # run
//!
//! A simple scripting language for CLI automation, inspired by shell scripting and Makefiles.
//! Define functions in a `Runfile` (or `~/.runfile`) and call them from the command line to streamline your development workflow.
//!
//! ## Usage
//!
//! - Run a script file: `run myscript.run`
//! - Call a function: `run build`, `run docker shell app`
//! - Pass arguments: `run start dev`, `run git commit "Initial commit"`
//! - Interactive shell: `run`
//!
//! See README.md for more details and examples.

use clap::Parser as ClapParser;
use run::{completion, config, executor, mcp, repl};
use std::path::PathBuf;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// CLI arguments for the run tool.
#[derive(ClapParser)]
#[command(name = "run")]
#[command(version = PKG_VERSION)]
#[command(about = "A simple scripting language for CLI automation", long_about = None)]
struct Cli {
    /// Script file to execute, or function name to call
    #[arg(value_name = "FILE_OR_FUNCTION")]
    first_arg: Option<String>,

    /// Additional arguments for function calls
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,

    /// List all available functions from the Runfile
    #[arg(short, long)]
    list: bool,

    /// Generate shell completion script
    #[arg(long, value_name = "SHELL")]
    generate_completion: Option<completion::Shell>,

    /// Install shell completion (automatically detects shell and updates config)
    #[arg(long, value_name = "SHELL")]
    install_completion: Option<Option<completion::Shell>>,

    /// Inspect and output JSON schema for all functions
    #[arg(long)]
    inspect: bool,

    /// Start MCP server for AI agent integration
    #[arg(long)]
    serve_mcp: bool,
}

/// Entry point for the CLI tool.
fn main() {
    let cli = Cli::parse();

    // Handle --install-completion flag
    if let Some(shell_opt) = cli.install_completion {
        completion::install_completion_interactive(shell_opt, config::get_home_dir);
        return;
    }

    // Handle --generate-completion flag
    if let Some(shell) = cli.generate_completion {
        completion::generate_completion_script(shell);
        return;
    }

    // Handle --list flag
    if cli.list {
        executor::list_functions();
        return;
    }

    // Handle --inspect flag
    if cli.inspect {
        mcp::print_inspect();
        return;
    }

    // Handle --serve-mcp flag
    if cli.serve_mcp {
        mcp::serve_mcp();
        return;
    }

    match cli.first_arg {
        Some(first_arg) => {
            // Check if it's a file that exists
            let path = PathBuf::from(&first_arg);
            if path.exists() && path.is_file() {
                // File mode: read and execute script
                executor::execute_file(&path);
            } else {
                // Function call mode: load config and call function with args
                executor::run_function_call(&first_arg, &cli.args);
            }
        }
        None => {
            // REPL mode: interactive shell
            repl::run_repl();
        }
    }
}
