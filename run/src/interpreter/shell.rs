//! Shell command execution and interpreter resolution

use crate::ast::{Attribute, ShellType, CommandOutput};
use crate::transpiler::Interpreter as TranspilerInterpreter;
use std::process::{Command, Stdio};
use std::time::{SystemTime, Instant, UNIX_EPOCH};

/// Get the Python executable (prefers python3)
pub(super) fn get_python_executable() -> String {
    if which::which("python3").is_ok() {
        "python3".to_string()
    } else {
        "python".to_string()
    }
}

/// Map a TranspilerInterpreter to shell command and argument
/// Returns (shell_command, shell_arg, interpreter_name)
pub(super) fn interpreter_to_shell_args(interpreter: &TranspilerInterpreter) -> (String, &'static str, &'static str) {
    match interpreter {
        TranspilerInterpreter::Sh => ("sh".to_string(), "-c", "sh"),
        TranspilerInterpreter::Bash => ("bash".to_string(), "-c", "bash"),
        TranspilerInterpreter::Pwsh => ("pwsh".to_string(), "-Command", "pwsh"),
        TranspilerInterpreter::Python => (get_python_executable(), "-c", "python"),
        TranspilerInterpreter::Python3 => ("python3".to_string(), "-c", "python3"),
        TranspilerInterpreter::Node => ("node".to_string(), "-e", "node"),
        TranspilerInterpreter::Ruby => ("ruby".to_string(), "-e", "ruby"),
    }
}

/// Execute a command and capture its output
pub(super) fn execute_with_capture(
    command: &str,
    shell_cmd: &str,
    shell_arg: &str,
    display_command: Option<&str>,
) -> Result<CommandOutput, Box<dyn std::error::Error>> {
    execute_with_capture_and_args(command, shell_cmd, shell_arg, &[], display_command)
}

/// Execute a command and capture its output, with additional arguments
/// Arguments are passed after the script for polyglot languages (Python, Node, Ruby)
/// The display_command is used for output/logging instead of the full script (which may include preamble)
pub(super) fn execute_with_capture_and_args(
    command: &str,
    shell_cmd: &str,
    shell_arg: &str,
    args: &[String],
    display_command: Option<&str>,
) -> Result<CommandOutput, Box<dyn std::error::Error>> {
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis();
    let start = Instant::now();

    let mut cmd = Command::new(shell_cmd);
    cmd.arg(shell_arg).arg(command);

    // Pass additional arguments after the script
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output()?;

    Ok(CommandOutput {
        // Use display_command if provided, otherwise fall back to the full command
        command: display_command.unwrap_or(command).to_string(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code(),
        duration_ms: start.elapsed().as_millis(),
        started_at,
    })
}

/// Execute a script in a single shell invocation
pub(super) fn execute_single_shell_invocation(
    script: &str,
    interpreter: &TranspilerInterpreter,
) -> Result<(), Box<dyn std::error::Error>> {
    let (shell_cmd, shell_arg, _) = interpreter_to_shell_args(interpreter);

    let status = Command::new(&shell_cmd)
        .arg(shell_arg)
        .arg(script)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        return Err(format!("Command failed with status: {}", status).into());
    }

    Ok(())
}

/// Execute a command with optional shell attributes and arguments
pub(super) fn execute_command_with_args(
    command: &str,
    attributes: &[Attribute],
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if there's a custom shell attribute
    let shell_attr: Option<&ShellType> = attributes
        .iter()
        .find_map(|attr| match attr {
            Attribute::Shell(shell) => Some(shell),
            _ => None,
        });

    let (shell_cmd, shell_arg) = if let Some(shell_type) = shell_attr {
        // Use the specified shell from attributes
        match shell_type {
            ShellType::Python => (get_python_executable(), "-c".to_string()),
            ShellType::Python3 => ("python3".to_string(), "-c".to_string()),
            ShellType::Node => ("node".to_string(), "-e".to_string()),
            ShellType::Ruby => ("ruby".to_string(), "-e".to_string()),
            ShellType::Pwsh => ("pwsh".to_string(), "-c".to_string()),
            ShellType::Bash => ("bash".to_string(), "-c".to_string()),
            ShellType::Sh => ("sh".to_string(), "-c".to_string()),
        }
    } else {
        // Check for RUN_SHELL environment variable, otherwise use platform defaults
        let shell = if let Ok(custom_shell) = std::env::var("RUN_SHELL") {
            custom_shell
        } else if cfg!(target_os = "windows") {
            // Default to PowerShell on Windows
            // Try to find pwsh (PowerShell 7+) first, then fallback to powershell (Windows PowerShell)
            if which::which("pwsh").is_ok() {
                "pwsh".to_string()
            } else {
                "powershell".to_string()
            }
        } else {
            // Default to sh on Unix-like systems
            "sh".to_string()
        };
        (shell, "-c".to_string())
    };

    let mut cmd = Command::new(&shell_cmd);
    cmd.arg(&shell_arg).arg(command);

    // For custom shells with arguments, pass them after the script
    // This makes them available as sys.argv[1:], process.argv[2:], etc.
    if shell_attr.is_some() {
        for arg in args {
            cmd.arg(arg);
        }
    }

    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        eprintln!("Command failed with status: {}", status);
    }

    Ok(())
}

/// Execute a command with shell attributes (convenience wrapper)
pub(super) fn execute_command(
    command: &str,
    attributes: &[Attribute],
) -> Result<(), Box<dyn std::error::Error>> {
    execute_command_with_args(command, attributes, &[])
}


/// Resolve interpreter from shebang to ShellType
pub(super) fn resolve_shebang_interpreter(shebang: &str) -> Option<ShellType> {
    // Extract the binary name from the shebang
    let binary_name = if let Some(env_part) = shebang.strip_prefix("/usr/bin/env ") {
        // Format: #!/usr/bin/env python
        // Extract first word after "env"
        env_part.split_whitespace().next()?.to_string()
    } else {
        // Format: #!/bin/bash or #!/usr/bin/python3
        // Extract basename
        std::path::Path::new(shebang)
            .file_name()?
            .to_str()?
            .split_whitespace()
            .next()?
            .to_string()
    };

    // Map binary name to ShellType
    match binary_name.as_str() {
        "python" => Some(ShellType::Python),
        "python3" => Some(ShellType::Python3),
        "node" => Some(ShellType::Node),
        "ruby" => Some(ShellType::Ruby),
        "pwsh" | "powershell" => Some(ShellType::Pwsh),
        "bash" => Some(ShellType::Bash),
        "sh" => Some(ShellType::Sh),
        _ => None,  // Unknown interpreter
    }
}

/// Strip shebang line from function body
/// Removes the first shebang line (skipping comments before it)
pub(super) fn strip_shebang(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut result_lines = Vec::new();
    let mut found_shebang = false;

    for line in lines {
        let trimmed = line.trim();
        // Skip comments before shebang
        if !found_shebang && !trimmed.is_empty() && trimmed.starts_with('#') && !trimmed.starts_with("#!") {
            result_lines.push(line);
            continue;
        }
        // Skip the shebang line itself
        if !found_shebang && !trimmed.is_empty() && trimmed.starts_with("#!") {
            found_shebang = true;
            continue;
        }
        result_lines.push(line);
    }

    result_lines.join("\n")
}

/// Escape a string value for safe use in shell variable assignment
pub(super) fn escape_shell_value(value: &str) -> String {
    // Escape special shell characters
    value
        .replace('\\', "\\\\")  // Backslash must be first
        .replace('"', "\\\"")   // Double quotes
        .replace('$', "\\$")    // Dollar signs
        .replace('`', "\\`")    // Backticks
        .replace('!', "\\!")    // History expansion
}

/// Escape a string value for safe use in PowerShell variable assignment
pub(super) fn escape_pwsh_value(value: &str) -> String {
    // PowerShell uses backtick for escaping
    value
        .replace('`', "``")     // Backtick must be first
        .replace('"', "`\"")    // Double quotes
        .replace('$', "`$")     // Dollar signs
}
