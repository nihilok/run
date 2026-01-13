// Interpreter to execute the AST

use crate::ast::{Expression, Program, Statement};
use std::collections::HashMap;
use std::process::{Command, Stdio};

pub struct Interpreter {
    variables: HashMap<String, String>,
    functions: HashMap<String, Vec<Statement>>,
    simple_functions: HashMap<String, String>,
    block_functions: HashMap<String, Vec<String>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            simple_functions: HashMap::new(),
            block_functions: HashMap::new(),
        }
    }

    pub fn execute(&mut self, program: Program) -> Result<(), Box<dyn std::error::Error>> {
        for statement in program.statements {
            self.execute_statement(statement)?;
        }
        Ok(())
    }

    pub fn call_function_without_parens(
        &mut self,
        function_name: &str,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Strategy: try to match function names in different ways
        // 1. Direct match: "docker_shell" with args
        // 2. If args exist, try first arg as subcommand: "docker" + "shell" -> "docker:shell"
        // 3. Try replacing underscores with colons: "docker_shell" -> "docker:shell"

        // Try direct match first - simple functions
        if let Some(command_template) = self.simple_functions.get(function_name) {
            let command = self.substitute_args(command_template, args);
            return self.execute_command(&command);
        }

        // Try direct match - block functions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            return self.execute_block_commands(&commands, args);
        }

        // If we have args, try treating the first arg as a subcommand
        if !args.is_empty() {
            let nested_name = format!("{}:{}", function_name, args[0]);
            if let Some(command_template) = self.simple_functions.get(&nested_name) {
                let command = self.substitute_args(command_template, &args[1..]);
                return self.execute_command(&command);
            }
            if let Some(commands) = self.block_functions.get(&nested_name).cloned() {
                return self.execute_block_commands(&commands, &args[1..]);
            }
        }

        // Try replacing underscores with colons
        let with_colons = function_name.replace("_", ":");
        if with_colons != function_name {
            if let Some(command_template) = self.simple_functions.get(&with_colons) {
                let command = self.substitute_args(command_template, args);
                return self.execute_command(&command);
            }
            if let Some(commands) = self.block_functions.get(&with_colons).cloned() {
                return self.execute_block_commands(&commands, args);
            }
        }

        // Check for full function definitions
        if let Some(body) = self.functions.get(function_name).cloned() {
            for stmt in body {
                self.execute_statement(stmt)?;
            }
            return Ok(());
        }

        Err(format!("Function '{}' not found", function_name).into())
    }

    pub fn call_function_with_args(
        &mut self,
        function_name: &str,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Direct function call with args in parentheses
        // Try to find the function and execute it with substituted arguments

        if let Some(command_template) = self.simple_functions.get(function_name) {
            let command = self.substitute_args(command_template, args);
            return self.execute_command(&command);
        }

        // Check for block function definitions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            return self.execute_block_commands(&commands, args);
        }

        // Check for full function definitions
        if let Some(body) = self.functions.get(function_name).cloned() {
            for stmt in body {
                self.execute_statement(stmt)?;
            }
            return Ok(());
        }

        Err(format!("Function '{}' not found", function_name).into())
    }

    fn substitute_args(&self, template: &str, args: &[String]) -> String {
        let mut result = template.to_string();

        // First, handle ${N:-default} patterns (must be done before simple $N)
        // This regex-like approach handles bash default value syntax
        let mut i = 0;
        while i < 10 {
            // Handle ${N:-default} - use arg if provided, else use default
            let pattern_with_default = format!("${{{}:-", i + 1);
            while let Some(start) = result.find(&pattern_with_default) {
                // Find the closing brace
                if let Some(end_offset) = result[start..].find('}') {
                    let end = start + end_offset;
                    let default_value = &result[start + pattern_with_default.len()..end];
                    let replacement = if i < args.len() {
                        args[i].clone()
                    } else {
                        default_value.to_string()
                    };
                    result = format!("{}{}{}", &result[..start], replacement, &result[end + 1..]);
                } else {
                    break;
                }
            }

            // Handle ${N} without default - same as $N
            let pattern_braced = format!("${{{}}}",  i + 1);
            if let Some(arg) = args.get(i) {
                result = result.replace(&pattern_braced, arg);
            } else {
                result = result.replace(&pattern_braced, "");
            }

            i += 1;
        }

        // Replace simple $1, $2, $3, etc. with actual arguments
        for (i, arg) in args.iter().enumerate() {
            let placeholder = format!("${}", i + 1);
            result = result.replace(&placeholder, arg);
        }

        // Also support $@ for all arguments
        if result.contains("$@") {
            result = result.replace("$@", &args.join(" "));
        }

        // Replace user-defined variables (e.g., $myvar)
        for (var_name, var_value) in &self.variables {
            let placeholder = format!("${}", var_name);
            result = result.replace(&placeholder, var_value);
        }

        result
    }

    fn execute_statement(
        &mut self,
        statement: Statement,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match statement {
            Statement::Assignment { name, value } => {
                let Expression::String(val) = value;
                self.variables.insert(name, val);
            }
            Statement::SimpleFunctionDef {
                name,
                command_template,
            } => {
                self.simple_functions.insert(name, command_template);
            }
            Statement::BlockFunctionDef { name, commands } => {
                self.block_functions.insert(name, commands);
            }
            Statement::FunctionCall { name, args } => {
                // Call the function with the provided arguments
                self.call_function_with_args(&name, &args)?;
            }
            Statement::Command { command } => {
                // Substitute variables in the command before executing
                let substituted_command = self.substitute_args(&command, &[]);
                self.execute_command(&substituted_command)?;
            }
        }
        Ok(())
    }

    fn execute_block_commands(
        &self,
        commands: &[String],
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for cmd in commands {
            let substituted = self.substitute_args(cmd, args);
            self.execute_command(&substituted)?;
        }
        Ok(())
    }

    fn execute_command(&self, command: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Check for RUN_SHELL environment variable, otherwise use platform defaults
        let shell_cmd = if let Ok(custom_shell) = std::env::var("RUN_SHELL") {
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

        let status = Command::new(&shell_cmd)
            .arg("-c")
            .arg(command)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        if !status.success() {
            eprintln!("Command failed with status: {}", status);
        }

        Ok(())
    }
}
