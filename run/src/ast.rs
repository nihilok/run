// Abstract Syntax Tree definitions

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::OnceLock;

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
    #[must_use]
    pub fn from_outputs(
        function_name: &str,
        outputs: Vec<CommandOutput>,
        interpreter: &str,
    ) -> Self {
        let success = outputs.iter().all(|o| o.exit_code == Some(0));
        let total_duration_ms = outputs.iter().map(|o| o.duration_ms).sum();

        let summary = if success {
            format!(
                "Successfully executed {} with {} command(s)",
                function_name,
                outputs.len()
            )
        } else {
            format!("Execution of {function_name} failed")
        };

        // Try to extract SSH context from any of the commands
        let (remote_user, remote_host) = outputs
            .iter()
            .find_map(|o| ExecutionContext::extract_ssh_context(&o.command))
            .map_or((None, None), |(user, host)| (Some(user), Some(host)));

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
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Format as Markdown for LLM readability
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        // Header with context
        let _ = write!(md, "## Execution: `{}`\n\n", self.context.function_name);

        if let Some(host) = &self.context.remote_host {
            let _ = writeln!(
                md,
                "**Host:** {}@{}",
                self.context.remote_user.as_deref().unwrap_or("?"),
                host
            );
        }

        let _ = writeln!(
            md,
            "**Status:** {}",
            if self.success {
                "✓ Success"
            } else {
                "✗ Failed"
            }
        );
        let _ = write!(md, "**Duration:** {}ms\n\n", self.total_duration_ms);

        // Individual command outputs
        for (i, output) in self.outputs.iter().enumerate() {
            let _ = writeln!(md, "### Step {} ({}ms)", i + 1, output.duration_ms);
            let _ = write!(md, "`{}`\n\n", output.command);

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

            if let Some(code) = output.exit_code
                && code != 0
            {
                let _ = writeln!(md, "**Exit Code:** {code}");
            }
        }

        md
    }

    /// Format optimized for MCP tool response (clean markdown, no implementation details)
    /// This intentionally hides the command source code to protect sensitive information
    /// like database connection strings, API keys, etc.
    #[must_use]
    pub fn to_mcp_format(&self) -> String {
        let mut md = String::new();

        // Header with context
        let _ = write!(md, "## Execution: `{}`\n\n", self.context.function_name);

        if let Some(host) = &self.context.remote_host {
            let _ = writeln!(
                md,
                "**Host:** {}@{}",
                self.context.remote_user.as_deref().unwrap_or("?"),
                host
            );
        }

        let _ = writeln!(
            md,
            "**Status:** {}",
            if self.success {
                "✓ Success"
            } else {
                "✗ Failed"
            }
        );
        let _ = write!(md, "**Duration:** {}ms\n\n", self.total_duration_ms);

        // For MCP, we only show output, not implementation
        // Combine all outputs into a single section
        let all_stdout: String = self
            .outputs
            .iter()
            .filter(|o| !o.stdout.is_empty())
            .map(|o| o.stdout.as_str())
            .collect::<Vec<_>>()
            .join("");

        let all_stderr: String = self
            .outputs
            .iter()
            .filter(|o| !o.stderr.is_empty())
            .map(|o| o.stderr.as_str())
            .collect::<Vec<_>>()
            .join("");

        if !all_stdout.is_empty() {
            md.push_str("**Output:**\n```\n");
            md.push_str(&all_stdout);
            if !all_stdout.ends_with('\n') {
                md.push('\n');
            }
            md.push_str("```\n\n");
        }

        if !all_stderr.is_empty() {
            md.push_str("**Errors:**\n```\n");
            md.push_str(&all_stderr);
            if !all_stderr.ends_with('\n') {
                md.push('\n');
            }
            md.push_str("```\n\n");
        }

        // Show exit code if failed
        if !self.success
            && let Some(output) = self.outputs.last()
            && let Some(code) = output.exit_code
            && code != 0
        {
            let _ = writeln!(md, "**Exit Code:** {code}");
        }

        md
    }
}

/// Static regex for SSH context extraction (compiled once)
static SSH_REGEX: OnceLock<Regex> = OnceLock::new();

impl ExecutionContext {
    /// Parse SSH commands to extract remote execution context
    pub fn extract_ssh_context(command: &str) -> Option<(String, String)> {
        // Match patterns like:
        //   ssh user@host
        //   ssh -i key.pem user@host
        //   ssh -T -o LogLevel=QUIET user@host
        // The regex looks for "ssh" followed by optional flags, then user@host
        let regex = SSH_REGEX.get_or_init(|| {
            // This regex pattern is hardcoded and known to be valid
            match Regex::new(r"ssh\s+(?:-\S+\s+(?:\S+\s+)?)*(\w+)@([\w.-]+)") {
                Ok(r) => r,
                Err(_) => unreachable!("SSH regex pattern is hardcoded and valid"),
            }
        });
        let caps = regex.captures(command)?;

        Some((
            caps.get(1)?.as_str().to_string(), // user
            caps.get(2)?.as_str().to_string(), // host
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
    Unix, // Matches both Linux and MacOS
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ssh_context_basic() {
        let result = ExecutionContext::extract_ssh_context("ssh admin@webserver.example.com");
        assert!(result.is_some(), "Failed to match basic SSH command");
        let (user, host) = result.expect("Expected SSH context to be extracted");
        assert_eq!(user, "admin");
        assert_eq!(host, "webserver.example.com");
    }

    #[test]
    fn test_extract_ssh_context_with_key() {
        let result =
            ExecutionContext::extract_ssh_context("ssh -i ~/.ssh/key.pem ubuntu@192.168.1.1");
        assert!(result.is_some(), "Failed to match SSH with -i flag");
        let (user, host) = result.expect("Expected SSH context to be extracted");
        assert_eq!(user, "ubuntu");
        assert_eq!(host, "192.168.1.1");
    }

    #[test]
    fn test_extract_ssh_context_multiple_options() {
        let result =
            ExecutionContext::extract_ssh_context("ssh -T -o LogLevel=QUIET root@server.local");
        assert!(
            result.is_some(),
            "Failed to match SSH with multiple options"
        );
        let (user, host) = result.expect("Expected SSH context to be extracted");
        assert_eq!(user, "root");
        assert_eq!(host, "server.local");
    }

    #[test]
    fn test_extract_ssh_context_no_match() {
        let result = ExecutionContext::extract_ssh_context("echo hello");
        assert!(result.is_none());
    }

    #[test]
    fn test_output_mode_default() {
        assert_eq!(OutputMode::default(), OutputMode::Stream);
    }

    #[test]
    fn test_structured_result_from_outputs_success() {
        let outputs = vec![CommandOutput {
            command: "echo hello".to_string(),
            stdout: "hello\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: 10,
            started_at: 1000,
        }];

        let result = StructuredResult::from_outputs("test_fn", outputs, "sh");
        assert!(result.success);
        assert_eq!(result.total_duration_ms, 10);
        assert_eq!(result.context.function_name, "test_fn");
        assert_eq!(result.context.interpreter, "sh");
        assert!(result.context.remote_host.is_none());
        assert!(result.context.remote_user.is_none());
        assert!(result.summary.contains("Successfully executed"));
        assert_eq!(result.outputs.len(), 1);
    }

    #[test]
    fn test_structured_result_from_outputs_failure() {
        let outputs = vec![CommandOutput {
            command: "false".to_string(),
            stdout: String::new(),
            stderr: "error\n".to_string(),
            exit_code: Some(1),
            duration_ms: 5,
            started_at: 1000,
        }];

        let result = StructuredResult::from_outputs("failing_fn", outputs, "bash");
        assert!(!result.success);
        assert!(result.summary.contains("failed"));
    }

    #[test]
    fn test_structured_result_from_outputs_with_ssh() {
        let outputs = vec![CommandOutput {
            command: "ssh deploy@prod.server.com 'uptime'".to_string(),
            stdout: "up 10 days\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: 100,
            started_at: 1000,
        }];

        let result = StructuredResult::from_outputs("check_uptime", outputs, "sh");
        assert_eq!(result.context.remote_user.as_deref(), Some("deploy"));
        assert_eq!(
            result.context.remote_host.as_deref(),
            Some("prod.server.com")
        );
    }

    #[test]
    fn test_structured_result_from_outputs_multiple() {
        let outputs = vec![
            CommandOutput {
                command: "echo step1".to_string(),
                stdout: "step1\n".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                duration_ms: 5,
                started_at: 1000,
            },
            CommandOutput {
                command: "echo step2".to_string(),
                stdout: "step2\n".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                duration_ms: 10,
                started_at: 1005,
            },
        ];

        let result = StructuredResult::from_outputs("multi", outputs, "sh");
        assert!(result.success);
        assert_eq!(result.total_duration_ms, 15);
        assert_eq!(result.outputs.len(), 2);
        assert!(result.summary.contains("2 command(s)"));
    }

    #[test]
    fn test_structured_result_to_json() {
        let result = StructuredResult {
            context: ExecutionContext {
                function_name: "test".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![CommandOutput {
                command: "echo hi".to_string(),
                stdout: "hi\n".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                duration_ms: 5,
                started_at: 1000,
            }],
            success: true,
            total_duration_ms: 5,
            summary: "ok".to_string(),
        };

        let json = result.to_json();
        assert!(json.contains("\"function_name\": \"test\""));
        assert!(json.contains("\"success\": true"));
        assert!(json.contains("\"stdout\": \"hi\\n\""));
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Valid JSON");
        assert_eq!(parsed["success"], true);
    }

    #[test]
    fn test_structured_result_to_markdown() {
        let result = StructuredResult {
            context: ExecutionContext {
                function_name: "deploy".to_string(),
                remote_host: Some("server.com".to_string()),
                remote_user: Some("admin".to_string()),
                interpreter: "bash".to_string(),
                working_directory: None,
            },
            outputs: vec![CommandOutput {
                command: "deploy.sh".to_string(),
                stdout: "deployed\n".to_string(),
                stderr: "warning: slow\n".to_string(),
                exit_code: Some(0),
                duration_ms: 100,
                started_at: 1000,
            }],
            success: true,
            total_duration_ms: 100,
            summary: "ok".to_string(),
        };

        let md = result.to_markdown();
        assert!(md.contains("## Execution: `deploy`"));
        assert!(md.contains("**Host:** admin@server.com"));
        assert!(md.contains("✓ Success"));
        assert!(md.contains("**Duration:** 100ms"));
        assert!(md.contains("### Step 1"));
        assert!(md.contains("deployed"));
        assert!(md.contains("warning: slow"));
    }

    #[test]
    fn test_structured_result_to_markdown_failed_with_exit_code() {
        let result = StructuredResult {
            context: ExecutionContext {
                function_name: "fail".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![CommandOutput {
                command: "exit 42".to_string(),
                stdout: String::new(),
                stderr: "error\n".to_string(),
                exit_code: Some(42),
                duration_ms: 1,
                started_at: 1000,
            }],
            success: false,
            total_duration_ms: 1,
            summary: "failed".to_string(),
        };

        let md = result.to_markdown();
        assert!(md.contains("✗ Failed"));
        assert!(md.contains("**Exit Code:** 42"));
    }

    #[test]
    fn test_structured_result_to_mcp_format() {
        let result = StructuredResult {
            context: ExecutionContext {
                function_name: "test".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![
                CommandOutput {
                    command: "echo a".to_string(),
                    stdout: "a\n".to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                    duration_ms: 5,
                    started_at: 1000,
                },
                CommandOutput {
                    command: "echo b".to_string(),
                    stdout: "b\n".to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                    duration_ms: 5,
                    started_at: 1005,
                },
            ],
            success: true,
            total_duration_ms: 10,
            summary: "ok".to_string(),
        };

        let mcp = result.to_mcp_format();
        assert!(mcp.contains("## Execution: `test`"));
        assert!(mcp.contains("✓ Success"));
        // MCP format combines all stdout
        assert!(mcp.contains("a\n"));
        assert!(mcp.contains("b\n"));
        // MCP format should NOT show individual steps
        assert!(!mcp.contains("### Step"));
    }

    #[test]
    fn test_structured_result_to_mcp_format_failed() {
        let result = StructuredResult {
            context: ExecutionContext {
                function_name: "fail".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![CommandOutput {
                command: "false".to_string(),
                stdout: String::new(),
                stderr: "oh no\n".to_string(),
                exit_code: Some(1),
                duration_ms: 1,
                started_at: 1000,
            }],
            success: false,
            total_duration_ms: 1,
            summary: "failed".to_string(),
        };

        let mcp = result.to_mcp_format();
        assert!(mcp.contains("✗ Failed"));
        assert!(mcp.contains("oh no"));
        assert!(mcp.contains("**Exit Code:** 1"));
    }

    #[test]
    fn test_structured_result_to_markdown_no_host() {
        let result = StructuredResult {
            context: ExecutionContext {
                function_name: "local".to_string(),
                remote_host: None,
                remote_user: None,
                interpreter: "sh".to_string(),
                working_directory: None,
            },
            outputs: vec![],
            success: true,
            total_duration_ms: 0,
            summary: "ok".to_string(),
        };

        let md = result.to_markdown();
        assert!(!md.contains("**Host:**"));
    }
}
