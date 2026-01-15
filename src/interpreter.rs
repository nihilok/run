// Interpreter to execute the AST

use crate::ast::{Attribute, Expression, OsPlatform, Program, ShellType, Statement};
use std::collections::HashMap;
use std::process::{Command, Stdio};

#[derive(Clone)]
struct FunctionMetadata {
    attributes: Vec<Attribute>,
    shebang: Option<String>,
}

pub struct Interpreter {
    variables: HashMap<String, String>,
    functions: HashMap<String, Vec<Statement>>,
    simple_functions: HashMap<String, String>,
    block_functions: HashMap<String, Vec<String>>,
    function_metadata: HashMap<String, FunctionMetadata>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            simple_functions: HashMap::new(),
            block_functions: HashMap::new(),
            function_metadata: HashMap::new(),
        }
    }
    
    fn matches_current_platform(attributes: &[Attribute]) -> bool {
        // If no OS attributes, function is available on all platforms
        let os_attrs: Vec<&OsPlatform> = attributes
            .iter()
            .filter_map(|attr| match attr {
                Attribute::Os(platform) => Some(platform),
                _ => None,
            })
            .collect();
        
        if os_attrs.is_empty() {
            return true;
        }
        
        // Check if any of the OS attributes match the current platform
        os_attrs.iter().any(|platform| {
            match platform {
                OsPlatform::Windows => cfg!(target_os = "windows"),
                OsPlatform::Linux => cfg!(target_os = "linux"),
                OsPlatform::MacOS => cfg!(target_os = "macos"),
                OsPlatform::Unix => cfg!(unix),  // Matches Linux or macOS
            }
        })
    }

    // Helper to get attributes for simple functions (returns a slice reference)
    fn get_simple_function_attributes(&self, name: &str) -> &[Attribute] {
        self.function_metadata
            .get(name)
            .map(|m| m.attributes.as_slice())
            .unwrap_or(&[])
    }

    // Helper to get attributes and shebang for block functions
    fn get_block_function_metadata(&self, name: &str) -> (Vec<Attribute>, Option<&str>) {
        self.function_metadata.get(name).map_or_else(
            || (Vec::new(), None),
            |m| (m.attributes.clone(), m.shebang.as_deref())
        )
    }

    pub fn execute(&mut self, program: Program) -> Result<(), Box<dyn std::error::Error>> {
        for statement in program.statements {
            self.execute_statement(statement)?;
        }
        Ok(())
    }
    
    pub fn list_available_functions(&self) -> Vec<String> {
        let mut functions = Vec::new();
        
        // Collect simple functions
        for name in self.simple_functions.keys() {
            functions.push(name.clone());
        }
        
        // Collect block functions
        for name in self.block_functions.keys() {
            if !functions.contains(name) {
                functions.push(name.clone());
            }
        }
        
        // Collect full function definitions
        for name in self.functions.keys() {
            if !functions.contains(name) {
                functions.push(name.clone());
            }
        }
        
        // Sort for consistent output
        functions.sort();
        functions
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
            let attributes = self.get_simple_function_attributes(function_name);
            return self.execute_command(&command, attributes);
        }

        // Try direct match - block functions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            let (attributes, shebang) = self.get_block_function_metadata(function_name);
            return self.execute_block_commands(&commands, args, &attributes, shebang);
        }

        // If we have args, try treating the first arg as a subcommand
        if !args.is_empty() {
            let nested_name = format!("{}:{}", function_name, args[0]);
            if let Some(command_template) = self.simple_functions.get(&nested_name) {
                let command = self.substitute_args(command_template, &args[1..]);
                let attributes = self.get_simple_function_attributes(&nested_name);
                return self.execute_command(&command, attributes);
            }
            if let Some(commands) = self.block_functions.get(&nested_name).cloned() {
                let (attributes, shebang) = self.get_block_function_metadata(&nested_name);
                return self.execute_block_commands(&commands, &args[1..], &attributes, shebang);
            }
        }

        // Try replacing underscores with colons
        let with_colons = function_name.replace("_", ":");
        if with_colons != function_name {
            if let Some(command_template) = self.simple_functions.get(&with_colons) {
                let command = self.substitute_args(command_template, args);
                let attributes = self.get_simple_function_attributes(&with_colons);
                return self.execute_command(&command, attributes);
            }
            if let Some(commands) = self.block_functions.get(&with_colons).cloned() {
                let (attributes, shebang) = self.get_block_function_metadata(&with_colons);
                return self.execute_block_commands(&commands, args, &attributes, shebang);
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
            let attributes = self.get_simple_function_attributes(function_name);
            return self.execute_command(&command, attributes);
        }

        // Check for block function definitions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            let (attributes, shebang) = self.get_block_function_metadata(function_name);
            return self.execute_block_commands(&commands, args, &attributes, shebang);
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
                attributes,
            } => {
                // Only store function if it matches the current platform
                if Self::matches_current_platform(&attributes) {
                    self.simple_functions.insert(name.clone(), command_template);
                    self.function_metadata.insert(
                        name,
                        FunctionMetadata { 
                            attributes,
                            shebang: None,
                        },
                    );
                }
            }
            Statement::BlockFunctionDef { name, commands, attributes, shebang } => {
                // Only store function if it matches the current platform
                if Self::matches_current_platform(&attributes) {
                    self.block_functions.insert(name.clone(), commands);
                    self.function_metadata.insert(
                        name,
                        FunctionMetadata { 
                            attributes,
                            shebang: shebang.clone(),
                        },
                    );
                }
            }
            Statement::FunctionCall { name, args } => {
                // Call the function with the provided arguments
                self.call_function_with_args(&name, &args)?;
            }
            Statement::Command { command } => {
                // Substitute variables in the command before executing
                let substituted_command = self.substitute_args(&command, &[]);
                self.execute_command(&substituted_command, &[])?;
            }
        }
        Ok(())
    }

    fn execute_block_commands(
        &self,
        commands: &[String],
        args: &[String],
        attributes: &[Attribute],
        shebang: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check if there's a custom shell attribute
        let has_custom_shell = attributes
            .iter()
            .any(|attr| matches!(attr, Attribute::Shell(_)));
        
        if has_custom_shell {
            // For custom shells (Python, Node, Ruby, etc.), pass all commands as one block
            let full_script = commands.join("\n");
            let substituted = self.substitute_args(&full_script, args);
            self.execute_command_with_args(&substituted, attributes, args)?;
        } else if let Some(shebang_str) = shebang {
            // Use shebang if present (and no @shell attribute)
            if let Some(shell_type) = Self::resolve_shebang_interpreter(shebang_str) {
                // Create a synthetic Shell attribute from the shebang
                let shebang_attributes = vec![Attribute::Shell(shell_type)];
                let full_script = commands.join("\n");
                // Strip the shebang line from the script
                let stripped_script = Self::strip_shebang(&full_script);
                let substituted = self.substitute_args(&stripped_script, args);
                self.execute_command_with_args(&substituted, &shebang_attributes, args)?;
            } else {
                // Unknown interpreter - fall back to default shell with warning
                eprintln!("Warning: Unknown interpreter in shebang '{}'. Falling back to default shell.", shebang_str);
                // Execute with regular shell
                for cmd in commands {
                    let substituted = self.substitute_args(cmd, args);
                    self.execute_command_with_args(&substituted, attributes, &[])?;
                }
            }
        } else {
            // For regular shell, execute commands one by one
            for cmd in commands {
                let substituted = self.substitute_args(cmd, args);
                self.execute_command_with_args(&substituted, attributes, &[])?;
            }
        }
        Ok(())
    }

    fn execute_command(&self, command: &str, attributes: &[Attribute]) -> Result<(), Box<dyn std::error::Error>> {
        self.execute_command_with_args(command, attributes, &[])
    }
    
    // Helper function to get the Python executable (prefers python3)
    fn get_python_executable() -> String {
        if which::which("python3").is_ok() {
            "python3".to_string()
        } else {
            "python".to_string()
        }
    }

    // Resolve interpreter from shebang to ShellType
    fn resolve_shebang_interpreter(shebang: &str) -> Option<ShellType> {
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

    // Strip shebang line from function body
    // Removes the first shebang line (skipping comments before it)
    fn strip_shebang(body: &str) -> String {
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

    fn execute_command_with_args(&self, command: &str, attributes: &[Attribute], args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
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
                ShellType::Python => (Self::get_python_executable(), "-c".to_string()),
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
}
