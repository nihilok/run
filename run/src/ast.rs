// Abstract Syntax Tree definitions

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// Output capture mode for command execution
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OutputMode {
    /// Stream directly to terminal (current behavior, default for CLI)
    #[default]
    Stream,

    /// Capture output and also print to terminal (for programmatic access with live output)
    /// Not exposed via CLI, but available for library use
    Capture,

    /// Capture output silently and format as structured result (for MCP/JSON/Markdown)
    /// Output is suppressed during execution and only the formatted result is printed at the end
    Structured,
}

/// Result of a single command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    /// The command that was executed
    pub command: String,

    /// Captured standard output
    pub stdout: String,

    /// Captured standard error
    pub stderr: String,

    /// Process exit code (None if killed by signal)
    pub exit_code: Option<i32>,

    /// Execution duration in milliseconds
    pub duration_ms: u128,

    /// Timestamp when execution started (Unix epoch ms)
    pub started_at: u128,
}

/// Context information about command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// The Runfile function name that was invoked
    pub function_name: String,

    /// Remote host if SSH detected (extracted from command)
    pub remote_host: Option<String>,

    /// Remote user if SSH detected
    pub remote_user: Option<String>,

    /// Shell/interpreter used (@shell attribute value)
    pub interpreter: String,

    /// Working directory
    pub working_directory: Option<String>,
}

/// Complete structured result for MCP responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredResult {
    /// Execution context
    pub context: ExecutionContext,

    /// Individual command outputs (in execution order)
    pub outputs: Vec<CommandOutput>,

    /// Overall success (all commands exited 0)
    pub success: bool,

    /// Total execution time
    pub total_duration_ms: u128,

    /// Human-readable summary
    pub summary: String,
}

impl StructuredResult {
    /// Create from a collection of command outputs
    pub fn from_outputs(function_name: &str, outputs: Vec<CommandOutput>, interpreter: &str) -> Self {
        let success = outputs.iter().all(|o| o.exit_code == Some(0));
        let total_duration_ms = outputs.iter().map(|o| o.duration_ms).sum();

        let summary = if success {
            format!("Successfully executed {} with {} command(s)", function_name, outputs.len())
        } else {
            format!("Execution of {} failed", function_name)
        };

        // Try to extract SSH context from any of the commands
        let (remote_user, remote_host) = outputs
            .iter()
            .find_map(|o| ExecutionContext::extract_ssh_context(&o.command))
            .map(|(user, host)| (Some(user), Some(host)))
            .unwrap_or((None, None));

        Self {
            context: ExecutionContext {
                function_name: function_name.to_string(),
                remote_host,
                remote_user,
                interpreter: interpreter.to_string(),
                working_directory: std::env::current_dir()
                    .ok()
                    .and_then(|p| p.to_str().map(String::from)),
            },
            outputs,
            success,
            total_duration_ms,
            summary,
        }
    }

    /// Format as JSON for programmatic consumption
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Format as Markdown for LLM readability
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        // Header with context
        md.push_str(&format!("## Execution: `{}`\n\n", self.context.function_name));

        if let Some(host) = &self.context.remote_host {
            md.push_str(&format!("**Host:** {}@{}\n",
                self.context.remote_user.as_deref().unwrap_or("?"),
                host
            ));
        }

        md.push_str(&format!("**Status:** {}\n",
            if self.success { "✓ Success" } else { "✗ Failed" }
        ));
        md.push_str(&format!("**Duration:** {}ms\n\n", self.total_duration_ms));

        // Individual command outputs
        for (i, output) in self.outputs.iter().enumerate() {
            md.push_str(&format!("### Step {} ({}ms)\n", i + 1, output.duration_ms));
            md.push_str(&format!("`{}`\n\n", output.command));

            if !output.stdout.is_empty() {
                md.push_str("**Output:**\n```\n");
                md.push_str(&output.stdout);
                md.push_str("```\n\n");
            }

            if !output.stderr.is_empty() {
                md.push_str("**Errors:**\n```\n");
                md.push_str(&output.stderr);
                md.push_str("```\n\n");
            }

            if let Some(code) = output.exit_code {
                if code != 0 {
                    md.push_str(&format!("**Exit Code:** {}\n\n", code));
                }
            }
        }

        md
    }

    /// Format optimized for MCP tool response (clean markdown, no JSON)
    pub fn to_mcp_format(&self) -> String {
        self.to_markdown()
    }
}

/// Static regex for SSH context extraction (compiled once)
static SSH_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"ssh\s+(?:-\S+\s+(?:\S+\s+)?)*(\w+)@([\w.-]+)")
        .expect("SSH regex pattern is valid")
});

impl ExecutionContext {
    /// Parse SSH commands to extract remote execution context
    pub fn extract_ssh_context(command: &str) -> Option<(String, String)> {
        // Match patterns like:
        //   ssh user@host
        //   ssh -i key.pem user@host
        //   ssh -T -o LogLevel=QUIET user@host
        // The regex looks for "ssh" followed by optional flags, then user@host
        let caps = SSH_REGEX.captures(command)?;

        Some((
            caps.get(1)?.as_str().to_string(),  // user
            caps.get(2)?.as_str().to_string(),  // host
        ))
    }
}

/// Function parameter definition
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: ArgType,
    pub default_value: Option<String>,
    pub is_rest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Assignment {
        name: String,
        value: Expression,
    },
    SimpleFunctionDef {
        name: String,
        params: Vec<Parameter>,
        command_template: String,
        attributes: Vec<Attribute>,
    },
    BlockFunctionDef {
        name: String,
        params: Vec<Parameter>,
        commands: Vec<String>,
        attributes: Vec<Attribute>,
        shebang: Option<String>,
    },
    FunctionCall {
        name: String,
        args: Vec<String>,
    },
    Command {
        command: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Attribute {
    Os(OsPlatform),
    Shell(ShellType),
    Desc(String),
    Arg(ArgMetadata),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArgMetadata {
    pub position: usize,
    pub name: String,
    pub arg_type: ArgType,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgType {
    String,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OsPlatform {
    Windows,
    Linux,
    MacOS,
    Unix,  // Matches both Linux and MacOS
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShellType {
    Python,
    Python3,
    Node,
    Ruby,
    Pwsh,
    Bash,
    Sh,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ssh_context_basic() {
        let result = ExecutionContext::extract_ssh_context("ssh admin@webserver.example.com");
        assert!(result.is_some(), "Failed to match basic SSH command");
        let (user, host) = result.unwrap();
        assert_eq!(user, "admin");
        assert_eq!(host, "webserver.example.com");
    }

    #[test]
    fn test_extract_ssh_context_with_key() {
        let result = ExecutionContext::extract_ssh_context("ssh -i ~/.ssh/key.pem ubuntu@192.168.1.1");
        assert!(result.is_some(), "Failed to match SSH with -i flag");
        let (user, host) = result.unwrap();
        assert_eq!(user, "ubuntu");
        assert_eq!(host, "192.168.1.1");
    }

    #[test]
    fn test_extract_ssh_context_multiple_options() {
        let result = ExecutionContext::extract_ssh_context("ssh -T -o LogLevel=QUIET root@server.local");
        assert!(result.is_some(), "Failed to match SSH with multiple options");
        let (user, host) = result.unwrap();
        assert_eq!(user, "root");
        assert_eq!(host, "server.local");
    }

    #[test]
    fn test_extract_ssh_context_no_match() {
        let result = ExecutionContext::extract_ssh_context("echo hello");
        assert!(result.is_none());
    }
}

