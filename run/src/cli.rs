//! CLI module containing the main entry point logic.
//!
//! This module is separated from main.rs to allow the runtool wrapper crate to reuse it.

use crate::{completion, config, executor, mcp, repl};
use clap::Parser as ClapParser;
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

    /// Output format for command execution (stream, json, markdown)
    #[arg(long, value_name = "FORMAT", default_value = "stream")]
    output_format: OutputFormatArg,

    /// Working directory containing the Runfile (alias: --runfile)
    #[arg(long = "working-dir", alias = "runfile", value_name = "PATH")]
    working_dir: Option<PathBuf>,
}

/// Output format for command execution
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormatArg {
    /// Stream output directly to terminal (default)
    Stream,
    /// Capture and output as JSON
    Json,
    /// Capture and output as Markdown
    Markdown,
}

impl OutputFormatArg {
    /// Get the output mode for this format
    pub fn mode(self) -> crate::ast::OutputMode {
        match self {
            Self::Stream => crate::ast::OutputMode::Stream,
            Self::Json | Self::Markdown => crate::ast::OutputMode::Structured,
        }
    }

    /// Format a structured result according to this format
    /// Returns None for Stream mode (no structured output)
    pub fn format_result(self, result: &crate::ast::StructuredResult) -> Option<String> {
        match self {
            Self::Stream => None,
            Self::Json => Some(result.to_json()),
            Self::Markdown => Some(result.to_mcp_format()),
        }
    }
}

/// Main CLI logic that can be called from external wrappers.
///
/// This function is public to allow the `runtool` wrapper crate to reuse the same logic.
pub fn run_cli() {
    let cli = Cli::parse();

    // Set custom working directory (Runfile location) if provided
    if let Some(ref runfile_path) = cli.working_dir {
        config::set_custom_runfile_path(Some(runfile_path.clone()));
    }

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
        mcp::tools::print_inspect();
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
                executor::run_function_call(&first_arg, &cli.args, cli.output_format);
            }
        }
        None => {
            // REPL mode: interactive shell
            repl::run_repl();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_stream_mode() {
        assert_eq!(
            OutputFormatArg::Stream.mode(),
            crate::ast::OutputMode::Stream
        );
    }

    #[test]
    fn test_output_format_json_mode() {
        assert_eq!(
            OutputFormatArg::Json.mode(),
            crate::ast::OutputMode::Structured
        );
    }

    #[test]
    fn test_output_format_markdown_mode() {
        assert_eq!(
            OutputFormatArg::Markdown.mode(),
            crate::ast::OutputMode::Structured
        );
    }

    #[test]
    fn test_format_result_stream_returns_none() {
        let result = crate::ast::StructuredResult {
            context: crate::ast::ExecutionContext {
                function_name: "test".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![],
            success: true,
            total_duration_ms: 0,
            summary: "test".to_string(),
        };
        assert!(OutputFormatArg::Stream.format_result(&result).is_none());
    }

    #[test]
    fn test_format_result_json_returns_json() {
        let result = crate::ast::StructuredResult {
            context: crate::ast::ExecutionContext {
                function_name: "test".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![],
            success: true,
            total_duration_ms: 0,
            summary: "test".to_string(),
        };
        let formatted = OutputFormatArg::Json.format_result(&result);
        assert!(formatted.is_some());
        let json_str = formatted.unwrap();
        assert!(json_str.contains("\"function_name\": \"test\""));
    }

    #[test]
    fn test_format_result_markdown_returns_markdown() {
        let result = crate::ast::StructuredResult {
            context: crate::ast::ExecutionContext {
                function_name: "test".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![],
            success: true,
            total_duration_ms: 0,
            summary: "test".to_string(),
        };
        let formatted = OutputFormatArg::Markdown.format_result(&result);
        assert!(formatted.is_some());
        let md = formatted.unwrap();
        assert!(md.contains("## Execution: `test`"));
    }
}
