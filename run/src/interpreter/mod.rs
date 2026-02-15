//! Interpreter to execute the AST
//!
//! This module provides the main interpreter for executing parsed Run scripts.
//! It handles function definitions, variable substitution, and command execution
//! with support for multiple interpreters (sh, bash, python, node, ruby, etc.)

mod execution;
mod preamble;
mod shell;

use crate::ast::{Attribute, CommandOutput, Expression, OutputMode, Program, Statement};
use crate::transpiler::{self, Interpreter as TranspilerInterpreter};
use crate::utils;
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct FunctionMetadata {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) shebang: Option<String>,
    pub(crate) params: Vec<crate::ast::Parameter>,
}

pub struct Interpreter {
    variables: HashMap<String, String>,
    functions: HashMap<String, Vec<Statement>>,
    simple_functions: HashMap<String, String>,
    block_functions: HashMap<String, Vec<String>>,
    function_metadata: HashMap<String, FunctionMetadata>,
    /// Output capture mode
    output_mode: OutputMode,
    /// Captured outputs when in Capture/Structured mode
    captured_outputs: Vec<CommandOutput>,
    /// Last interpreter used (for structured output context)
    last_interpreter_name: String,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            simple_functions: HashMap::new(),
            block_functions: HashMap::new(),
            function_metadata: HashMap::new(),
            output_mode: OutputMode::default(),
            captured_outputs: Vec::new(),
            last_interpreter_name: "sh".to_string(),
        }
    }
}

/// Shell-quote a slice of arguments so each remains a separate word when
/// substituted into a shell command string via text replacement.
fn shell_quote_args(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.is_empty() {
                "''".to_string()
            } else if a.bytes().all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' | b':' | b'=' | b'+' | b'@' | b'%' | b',')) {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Interpreter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the output capture mode
    pub fn set_output_mode(&mut self, mode: OutputMode) {
        self.output_mode = mode;
    }

    /// Get the current output mode
    #[must_use]
    pub fn output_mode(&self) -> OutputMode {
        self.output_mode
    }

    /// Take captured outputs (clears the internal buffer)
    pub fn take_captured_outputs(&mut self) -> Vec<CommandOutput> {
        std::mem::take(&mut self.captured_outputs)
    }

    /// Get the last interpreter used
    #[must_use]
    pub fn last_interpreter(&self) -> &str {
        &self.last_interpreter_name
    }

    /// Add a captured output
    pub(crate) fn add_captured_output(&mut self, output: CommandOutput) {
        self.captured_outputs.push(output);
    }

    // Helper to get attributes for simple functions (returns a slice reference)
    fn get_simple_function_attributes(&self, name: &str) -> &[Attribute] {
        self.function_metadata
            .get(name)
            .map_or(&[], |m| m.attributes.as_slice())
    }

    // Helper to get attributes and shebang for block functions
    fn get_block_function_metadata(&self, name: &str) -> (Vec<Attribute>, Option<&str>) {
        self.function_metadata.get(name).map_or_else(
            || (Vec::new(), None),
            |m| (m.attributes.clone(), m.shebang.as_deref()),
        )
    }

    /// Execute a parsed program
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - A statement fails to execute
    /// - A function call references a non-existent function
    /// - A command execution fails
    pub fn execute(&mut self, program: Program) -> Result<(), Box<dyn std::error::Error>> {
        for statement in program.statements {
            self.execute_statement(statement)?;
        }
        Ok(())
    }

    #[must_use]
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

    /// Call a function without parentheses, trying multiple name resolution strategies
    ///
    /// This method attempts to match function names in different ways:
    /// 1. Direct match: `docker_shell` with args
    /// 2. If args exist, try first arg as subcommand: `docker` + `shell` -> `docker:shell`
    /// 3. Try replacing underscores with colons: `docker_shell` -> `docker:shell`
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - The function is not found after trying all resolution strategies
    /// - The function execution fails
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
        if let Some(command_template) = self.simple_functions.get(function_name).cloned() {
            let attributes = self.get_simple_function_attributes(function_name).to_vec();
            return self.execute_simple_function(
                function_name,
                &command_template,
                args,
                &attributes,
            );
        }

        // Try direct match - block functions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            let (attributes, shebang) = self.get_block_function_metadata(function_name);
            let shebang_owned = shebang.map(String::from);
            return self.execute_block_commands(
                function_name,
                &commands,
                args,
                &attributes,
                shebang_owned.as_deref(),
            );
        }

        // If we have args, try treating the first arg as a subcommand
        if !args.is_empty() {
            let nested_name = format!("{}:{}", function_name, args[0]);
            if let Some(command_template) = self.simple_functions.get(&nested_name).cloned() {
                let attributes = self.get_simple_function_attributes(&nested_name).to_vec();
                return self.execute_simple_function(
                    &nested_name,
                    &command_template,
                    &args[1..],
                    &attributes,
                );
            }
            if let Some(commands) = self.block_functions.get(&nested_name).cloned() {
                let (attributes, shebang) = self.get_block_function_metadata(&nested_name);
                let shebang_owned = shebang.map(String::from);
                return self.execute_block_commands(
                    &nested_name,
                    &commands,
                    &args[1..],
                    &attributes,
                    shebang_owned.as_deref(),
                );
            }
        }

        // Try replacing double underscores with colons (MCP sanitization convention)
        // e.g., "nested__function" -> "nested:function"
        if function_name.contains("__") {
            let with_colons = function_name.replace("__", ":");
            if let Some(command_template) = self.simple_functions.get(&with_colons).cloned() {
                let attributes = self.get_simple_function_attributes(&with_colons).to_vec();
                return self.execute_simple_function(
                    &with_colons,
                    &command_template,
                    args,
                    &attributes,
                );
            }
            if let Some(commands) = self.block_functions.get(&with_colons).cloned() {
                let (attributes, shebang) = self.get_block_function_metadata(&with_colons);
                let shebang_owned = shebang.map(String::from);
                return self.execute_block_commands(
                    &with_colons,
                    &commands,
                    args,
                    &attributes,
                    shebang_owned.as_deref(),
                );
            }
        }

        // Try replacing underscores with colons
        let with_colons = function_name.replace('_', ":");
        if with_colons != function_name {
            if let Some(command_template) = self.simple_functions.get(&with_colons).cloned() {
                let attributes = self.get_simple_function_attributes(&with_colons).to_vec();
                return self.execute_simple_function(
                    &with_colons,
                    &command_template,
                    args,
                    &attributes,
                );
            }
            if let Some(commands) = self.block_functions.get(&with_colons).cloned() {
                let (attributes, shebang) = self.get_block_function_metadata(&with_colons);
                let shebang_owned = shebang.map(String::from);
                return self.execute_block_commands(
                    &with_colons,
                    &commands,
                    args,
                    &attributes,
                    shebang_owned.as_deref(),
                );
            }
        }

        // Check for full function definitions
        if let Some(body) = self.functions.get(function_name).cloned() {
            for stmt in body {
                self.execute_statement(stmt)?;
            }
            return Ok(());
        }

        Err(format!("Function '{function_name}' not found").into())
    }

    /// Call a function with explicit arguments (parentheses syntax)
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - The specified function is not found
    /// - The function execution fails
    pub fn call_function_with_args(
        &mut self,
        function_name: &str,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Direct function call with args in parentheses
        // Try to find the function and execute it with substituted arguments

        if let Some(command_template) = self.simple_functions.get(function_name).cloned() {
            let attributes = self.get_simple_function_attributes(function_name).to_vec();
            return self.execute_simple_function(
                function_name,
                &command_template,
                args,
                &attributes,
            );
        }

        // Check for block function definitions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            let (attributes, shebang) = self.get_block_function_metadata(function_name);
            let shebang_owned = shebang.map(String::from);
            return self.execute_block_commands(
                function_name,
                &commands,
                args,
                &attributes,
                shebang_owned.as_deref(),
            );
        }

        // Check for full function definitions
        if let Some(body) = self.functions.get(function_name).cloned() {
            for stmt in body {
                self.execute_statement(stmt)?;
            }
            return Ok(());
        }

        Err(format!("Function '{function_name}' not found").into())
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
            let pattern_braced = format!("${{{}}}", i + 1);
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
        // Shell-quote each arg individually so they remain separate words
        if result.contains("$@") {
            let quoted = shell_quote_args(args);
            // Replace "$@" (with surrounding quotes) first, then bare $@
            result = result.replace("\"$@\"", &quoted);
            result = result.replace("$@", &quoted);
        }

        // Replace user-defined variables (e.g., $myvar)
        for (var_name, var_value) in &self.variables {
            let placeholder = format!("${var_name}");
            result = result.replace(&placeholder, var_value);
        }

        result
    }

    /// Substitute arguments using parameter definitions
    /// If params are defined, use named substitution; otherwise fall back to positional
    fn substitute_args_with_params(
        &self,
        template: &str,
        args: &[String],
        params: &[crate::ast::Parameter],
    ) -> String {
        if params.is_empty() {
            return self.substitute_args(template, args);
        }

        let mut result = template.to_string();

        for (i, param) in params.iter().enumerate() {
            if param.is_rest {
                // Rest parameter: shell-quote all remaining args individually
                let rest_args = if i < args.len() {
                    shell_quote_args(&args[i..])
                } else {
                    String::new()
                };
                result = result.replace(&format!("${}", param.name), &rest_args);
                result = result.replace(&format!("${{{}}}", param.name), &rest_args);
                // Also support "$@" and $@ for rest parameters
                result = result.replace("\"$@\"", &rest_args);
                result = result.replace("$@", &rest_args);
            } else {
                let value = if i < args.len() {
                    &args[i]
                } else if let Some(default) = &param.default_value {
                    default
                } else {
                    eprintln!("Warning: Missing required argument: {}", param.name);
                    ""
                };

                // Replace both $name and ${name} and $N (for backward compatibility)
                result = result.replace(&format!("${}", param.name), value);
                result = result.replace(&format!("${{{}}}", param.name), value);
                result = result.replace(&format!("${}", i + 1), value); // Also support positional
            }
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
                params,
                command_template,
                attributes,
            } => {
                // Only store function if it matches the current platform
                if utils::matches_current_platform(&attributes) {
                    self.simple_functions.insert(name.clone(), command_template);
                    self.function_metadata.insert(
                        name,
                        FunctionMetadata {
                            attributes,
                            shebang: None,
                            params,
                        },
                    );
                }
            }
            Statement::BlockFunctionDef {
                name,
                params,
                commands,
                attributes,
                shebang,
            } => {
                // Only store function if it matches the current platform
                if utils::matches_current_platform(&attributes) {
                    self.block_functions.insert(name.clone(), commands);
                    self.function_metadata.insert(
                        name,
                        FunctionMetadata {
                            attributes,
                            shebang: shebang.clone(),
                            params,
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
                shell::execute_command(&substituted_command, &[])?;
            }
        }
        Ok(())
    }

    /// Resolve the interpreter for a given function
    fn resolve_function_interpreter(
        attributes: &[Attribute],
        shebang: Option<&str>,
    ) -> TranspilerInterpreter {
        // Check for @shell attribute
        for attr in attributes {
            if let Attribute::Shell(shell_type) = attr {
                return TranspilerInterpreter::from_shell_type(shell_type);
            }
        }

        // Check for shebang
        #[allow(clippy::collapsible_if)]
        if let Some(shebang_str) = shebang {
            if let Some(shell_type) = shell::resolve_shebang_interpreter(shebang_str) {
                return TranspilerInterpreter::from_shell_type(&shell_type);
            }
        }

        // Default to platform default
        TranspilerInterpreter::default()
    }

    /// Execute a simple function with preambles for composition
    fn execute_simple_function(
        &mut self,
        target_name: &str,
        command_template: &str,
        args: &[String],
        attributes: &[Attribute],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Determine the target interpreter
        let target_interpreter = Self::resolve_function_interpreter(attributes, None);

        // Create closure for resolve_interpreter
        let resolve_interpreter = |name: &str, attrs: &[Attribute], shebang: Option<&str>| {
            let _ = name;
            Self::resolve_function_interpreter(attrs, shebang)
        };

        // Collect all rewritable sibling names
        let rewritable_names = execution::collect_rewritable_siblings(
            target_name,
            &target_interpreter,
            &self.simple_functions,
            &self.block_functions,
            &self.function_metadata,
            &resolve_interpreter,
        );
        let sibling_names: Vec<&str> = rewritable_names.iter().map(String::as_str).collect();

        // Rewrite call sites in the command template
        let rewritten_body = transpiler::rewrite_call_sites(command_template, &sibling_names);

        // Build preambles
        let var_preamble = preamble::build_variable_preamble(&self.variables, &target_interpreter);
        let func_preamble = preamble::build_function_preamble(
            target_name,
            &target_interpreter,
            &self.simple_functions,
            &self.block_functions,
            &self.function_metadata,
            &resolve_interpreter,
        );

        // Combine preambles and body
        let combined_script =
            execution::build_combined_script(var_preamble, func_preamble, rewritten_body);

        // Get params from metadata for substitution
        let params = self
            .function_metadata
            .get(target_name)
            .map_or(&[] as &[crate::ast::Parameter], |m| m.params.as_slice());

        // Substitute args in both the combined script (for execution) and the original command (for display)
        let substituted = self.substitute_args_with_params(&combined_script, args, params);
        let display_cmd = self.substitute_args_with_params(command_template, args, params);
        self.execute_with_mode(&substituted, &target_interpreter, Some(&display_cmd))
    }

    fn execute_block_commands(
        &mut self,
        target_name: &str,
        commands: &[String],
        args: &[String],
        attributes: &[Attribute],
        shebang: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Determine the target interpreter
        let target_interpreter = Self::resolve_function_interpreter(attributes, shebang);

        // Get params from metadata for substitution
        let params: &[crate::ast::Parameter] = self
            .function_metadata
            .get(target_name)
            .map_or(&[] as &[crate::ast::Parameter], |m| m.params.as_slice());

        // Check if this is a polyglot language (Python, Node, Ruby)
        let is_polyglot = matches!(
            target_interpreter,
            TranspilerInterpreter::Python
                | TranspilerInterpreter::Python3
                | TranspilerInterpreter::Node
                | TranspilerInterpreter::Ruby
        );

        let full_script = commands.join("\n");

        if is_polyglot {
            let script = if shebang.is_some() {
                shell::strip_shebang(&full_script)
            } else {
                full_script
            };

            // Inject named arg variables from function parameters
            let arg_preamble = preamble::build_polyglot_arg_preamble(params, &target_interpreter);
            let script = if arg_preamble.is_empty() {
                script
            } else {
                format!("{arg_preamble}\n{script}")
            };

            let substituted = self.substitute_args_with_params(&script, args, params);

            // Use execute_with_mode_polyglot for proper capture support with args
            return self.execute_with_mode_polyglot(&substituted, &target_interpreter, args);
        }

        // For shell-compatible languages, build preamble and compose
        let resolve_interpreter = |name: &str, attrs: &[Attribute], shebang: Option<&str>| {
            let _ = name;
            Self::resolve_function_interpreter(attrs, shebang)
        };

        // Collect all rewritable sibling names
        let rewritable_names = execution::collect_rewritable_siblings(
            target_name,
            &target_interpreter,
            &self.simple_functions,
            &self.block_functions,
            &self.function_metadata,
            &resolve_interpreter,
        );
        let sibling_names: Vec<&str> = rewritable_names.iter().map(String::as_str).collect();

        // Rewrite call sites and build preambles
        let rewritten_body = transpiler::rewrite_call_sites(&full_script, &sibling_names);
        let var_preamble = preamble::build_variable_preamble(&self.variables, &target_interpreter);
        let func_preamble = preamble::build_function_preamble(
            target_name,
            &target_interpreter,
            &self.simple_functions,
            &self.block_functions,
            &self.function_metadata,
            &resolve_interpreter,
        );

        // Combine preambles and body
        let combined_script =
            execution::build_combined_script(var_preamble, func_preamble, rewritten_body);

        // Substitute args in both the combined script (for execution) and the original body (for display)
        let substituted = self.substitute_args_with_params(&combined_script, args, params);
        let display_cmd = self.substitute_args_with_params(&full_script, args, params);
        self.execute_with_mode(&substituted, &target_interpreter, Some(&display_cmd))
    }

    /// Execute a command with the current output mode
    /// The `display_command` is shown in structured output instead of the full script (which may include preamble)
    fn execute_with_mode(
        &mut self,
        script: &str,
        interpreter: &TranspilerInterpreter,
        display_command: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ast::OutputMode;

        // Track the interpreter for structured output context
        let (shell_cmd, shell_arg, interpreter_name) =
            shell::interpreter_to_shell_args(interpreter);
        self.last_interpreter_name = interpreter_name.to_string();

        match self.output_mode {
            OutputMode::Stream => {
                // Stream mode: execute normally without capture
                shell::execute_single_shell_invocation(script, interpreter)
            }
            OutputMode::Capture | OutputMode::Structured => {
                // Capture mode: use the shell args we already have
                self.execute_with_mode_custom(&shell_cmd, shell_arg, script, display_command)
            }
        }
    }

    /// Execute a command with custom shell and capture output
    fn execute_with_mode_custom(
        &mut self,
        shell_cmd: &str,
        shell_arg: &str,
        script: &str,
        display_command: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = shell::execute_with_capture(script, shell_cmd, shell_arg, display_command)?;

        // Only print output in Capture mode (not Structured, where we format it later)
        if matches!(self.output_mode, crate::ast::OutputMode::Capture) {
            if !output.stdout.is_empty() {
                print!("{}", output.stdout);
            }
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
            }
        }

        // Check for errors
        #[allow(clippy::collapsible_if)]
        if let Some(code) = output.exit_code {
            if code != 0 {
                self.add_captured_output(output);
                return Err(format!("Command failed with exit code: {code}").into());
            }
        }

        // Store the captured output
        self.add_captured_output(output);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn get_simple_functions(&self) -> &HashMap<String, String> {
        &self.simple_functions
    }

    #[cfg(test)]
    pub(crate) fn get_block_functions(&self) -> &HashMap<String, Vec<String>> {
        &self.block_functions
    }

    #[cfg(test)]
    pub(crate) fn get_variables(&self) -> &HashMap<String, String> {
        &self.variables
    }

    /// Execute a polyglot command with arguments (for Python, Node, Ruby)
    /// Arguments are passed as command-line arguments, accessible via sys.argv, process.argv, etc.
    fn execute_with_mode_polyglot(
        &mut self,
        script: &str,
        interpreter: &TranspilerInterpreter,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ast::OutputMode;

        // Track the interpreter for structured output context
        let (shell_cmd, shell_arg, interpreter_name) =
            shell::interpreter_to_shell_args(interpreter);
        self.last_interpreter_name = interpreter_name.to_string();

        match self.output_mode {
            OutputMode::Stream => {
                // Stream mode: execute with arguments using the original path
                let exec_attributes = vec![crate::ast::Attribute::Shell(match interpreter {
                    TranspilerInterpreter::Python => crate::ast::ShellType::Python,
                    TranspilerInterpreter::Python3 => crate::ast::ShellType::Python3,
                    TranspilerInterpreter::Node => crate::ast::ShellType::Node,
                    TranspilerInterpreter::Ruby => crate::ast::ShellType::Ruby,
                    _ => crate::ast::ShellType::Sh,
                })];
                shell::execute_command_with_args(script, &exec_attributes, args)
            }
            OutputMode::Capture | OutputMode::Structured => {
                // Capture mode: capture output with arguments
                // For polyglot, the script IS the user command (no preamble), so pass None
                let output = shell::execute_with_capture_and_args(
                    script, &shell_cmd, shell_arg, args, None,
                )?;

                // Only print output in Capture mode
                if matches!(self.output_mode, OutputMode::Capture) {
                    if !output.stdout.is_empty() {
                        print!("{}", output.stdout);
                    }
                    if !output.stderr.is_empty() {
                        eprint!("{}", output.stderr);
                    }
                }

                // Check for errors
                #[allow(clippy::collapsible_if)]
                if let Some(code) = output.exit_code {
                    if code != 0 {
                        self.add_captured_output(output);
                        return Err(format!("Command failed with exit code: {code}").into());
                    }
                }

                // Store the captured output
                self.add_captured_output(output);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ast::{
        Attribute, CommandOutput, Expression, OutputMode, Parameter, Program, ShellType, Statement,
    };

    #[test]
    fn test_shell_quote_args_empty_arg() {
        let args = vec![String::new()];
        assert_eq!(shell_quote_args(&args), "''");
    }

    #[test]
    fn test_shell_quote_args_safe_chars() {
        let args = vec!["hello".to_string(), "world".to_string()];
        assert_eq!(shell_quote_args(&args), "hello world");
    }

    #[test]
    fn test_shell_quote_args_special_chars() {
        let args = vec!["hello world".to_string()];
        assert_eq!(shell_quote_args(&args), "'hello world'");
    }

    #[test]
    fn test_shell_quote_args_single_quote() {
        let args = vec!["it's".to_string()];
        assert_eq!(shell_quote_args(&args), "'it'\\''s'");
    }

    #[test]
    fn test_shell_quote_args_mixed() {
        let args = vec![
            "simple".to_string(),
            "with space".to_string(),
            "--flag=value".to_string(),
        ];
        assert_eq!(shell_quote_args(&args), "simple 'with space' --flag=value");
    }

    #[test]
    fn test_shell_quote_args_empty_slice() {
        let args: Vec<String> = vec![];
        assert_eq!(shell_quote_args(&args), "");
    }

    #[test]
    fn test_interpreter_new_default() {
        let interp = Interpreter::new();
        assert_eq!(interp.output_mode(), OutputMode::Stream);
        assert_eq!(interp.last_interpreter(), "sh");
        assert!(interp.get_simple_functions().is_empty());
        assert!(interp.get_block_functions().is_empty());
        assert!(interp.get_variables().is_empty());
    }

    #[test]
    fn test_set_output_mode() {
        let mut interp = Interpreter::new();
        interp.set_output_mode(OutputMode::Capture);
        assert_eq!(interp.output_mode(), OutputMode::Capture);

        interp.set_output_mode(OutputMode::Structured);
        assert_eq!(interp.output_mode(), OutputMode::Structured);
    }

    #[test]
    fn test_take_captured_outputs() {
        let mut interp = Interpreter::new();
        let output = CommandOutput {
            command: "echo hi".to_string(),
            stdout: "hi\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: 10,
            started_at: 0,
        };
        interp.add_captured_output(output);

        let outputs = interp.take_captured_outputs();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].command, "echo hi");

        // After take, buffer should be empty
        let outputs2 = interp.take_captured_outputs();
        assert!(outputs2.is_empty());
    }

    #[test]
    fn test_execute_assignment() {
        let mut interp = Interpreter::new();
        let program = Program {
            statements: vec![Statement::Assignment {
                name: "MY_VAR".to_string(),
                value: Expression::String("hello".to_string()),
            }],
        };
        interp.execute(program).unwrap();
        assert_eq!(interp.get_variables().get("MY_VAR").unwrap(), "hello");
    }

    #[test]
    fn test_execute_simple_function_def() {
        let mut interp = Interpreter::new();
        let program = Program {
            statements: vec![Statement::SimpleFunctionDef {
                name: "greet".to_string(),
                params: vec![],
                command_template: "echo hello".to_string(),
                attributes: vec![],
            }],
        };
        interp.execute(program).unwrap();
        assert!(interp.get_simple_functions().contains_key("greet"));
        assert_eq!(
            interp.get_simple_functions().get("greet").unwrap(),
            "echo hello"
        );
    }

    #[test]
    fn test_execute_block_function_def() {
        let mut interp = Interpreter::new();
        let program = Program {
            statements: vec![Statement::BlockFunctionDef {
                name: "build".to_string(),
                params: vec![],
                commands: vec!["echo step1".to_string(), "echo step2".to_string()],
                attributes: vec![],
                shebang: None,
            }],
        };
        interp.execute(program).unwrap();
        assert!(interp.get_block_functions().contains_key("build"));
        assert_eq!(interp.get_block_functions()["build"].len(), 2);
    }

    #[test]
    fn test_list_available_functions_empty() {
        let interp = Interpreter::new();
        assert!(interp.list_available_functions().is_empty());
    }

    #[test]
    fn test_list_available_functions_sorted() {
        let mut interp = Interpreter::new();
        let program = Program {
            statements: vec![
                Statement::SimpleFunctionDef {
                    name: "zebra".to_string(),
                    params: vec![],
                    command_template: "echo z".to_string(),
                    attributes: vec![],
                },
                Statement::SimpleFunctionDef {
                    name: "alpha".to_string(),
                    params: vec![],
                    command_template: "echo a".to_string(),
                    attributes: vec![],
                },
                Statement::BlockFunctionDef {
                    name: "middle".to_string(),
                    params: vec![],
                    commands: vec!["echo m".to_string()],
                    attributes: vec![],
                    shebang: None,
                },
            ],
        };
        interp.execute(program).unwrap();

        let funcs = interp.list_available_functions();
        assert_eq!(funcs, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn test_call_function_not_found() {
        let mut interp = Interpreter::new();
        let result = interp.call_function_without_parens("nonexistent", &[]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Function 'nonexistent' not found")
        );
    }

    #[test]
    fn test_call_function_with_args_not_found() {
        let mut interp = Interpreter::new();
        let result = interp.call_function_with_args("nonexistent", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_substitute_args_positional() {
        let interp = Interpreter::new();
        let result =
            interp.substitute_args("echo $1 $2", &["hello".to_string(), "world".to_string()]);
        assert_eq!(result, "echo hello world");
    }

    #[test]
    fn test_substitute_args_all_args() {
        let interp = Interpreter::new();
        let result = interp.substitute_args("echo $@", &["a".to_string(), "b".to_string()]);
        assert_eq!(result, "echo a b");
    }

    #[test]
    fn test_substitute_args_default_value() {
        let interp = Interpreter::new();
        let result = interp.substitute_args("echo ${1:-default_val}", &[]);
        assert_eq!(result, "echo default_val");
    }

    #[test]
    fn test_substitute_args_default_value_with_arg() {
        let interp = Interpreter::new();
        let result = interp.substitute_args("echo ${1:-default_val}", &["provided".to_string()]);
        assert_eq!(result, "echo provided");
    }

    #[test]
    fn test_substitute_args_braced() {
        let interp = Interpreter::new();
        let result = interp.substitute_args("echo ${1} ${2}", &["a".to_string(), "b".to_string()]);
        assert_eq!(result, "echo a b");
    }

    #[test]
    fn test_substitute_args_braced_missing() {
        let interp = Interpreter::new();
        let result = interp.substitute_args("echo ${1} ${2}", &["a".to_string()]);
        assert_eq!(result, "echo a ");
    }

    #[test]
    fn test_substitute_args_with_variables() {
        let mut interp = Interpreter::new();
        interp
            .variables
            .insert("MY_VAR".to_string(), "value".to_string());
        let result = interp.substitute_args("echo $MY_VAR", &[]);
        assert_eq!(result, "echo value");
    }

    #[test]
    fn test_substitute_args_with_params_named() {
        let interp = Interpreter::new();
        let params = vec![
            Parameter {
                name: "name".to_string(),
                param_type: crate::ast::ArgType::String,
                default_value: None,
                is_rest: false,
            },
            Parameter {
                name: "greeting".to_string(),
                param_type: crate::ast::ArgType::String,
                default_value: Some("hello".to_string()),
                is_rest: false,
            },
        ];
        let result = interp.substitute_args_with_params(
            "echo $greeting $name",
            &["world".to_string()],
            &params,
        );
        assert_eq!(result, "echo hello world");
    }

    #[test]
    fn test_substitute_args_with_params_rest() {
        let interp = Interpreter::new();
        let params = vec![Parameter {
            name: "args".to_string(),
            param_type: crate::ast::ArgType::String,
            default_value: None,
            is_rest: true,
        }];
        let result = interp.substitute_args_with_params(
            "echo $args",
            &["a".to_string(), "b".to_string(), "c".to_string()],
            &params,
        );
        assert_eq!(result, "echo a b c");
    }

    #[test]
    fn test_substitute_args_with_params_fallback_positional() {
        let interp = Interpreter::new();
        // Empty params should fall back to positional substitution
        let result = interp.substitute_args_with_params(
            "echo $1 $2",
            &["hello".to_string(), "world".to_string()],
            &[],
        );
        assert_eq!(result, "echo hello world");
    }

    #[test]
    fn test_resolve_function_interpreter_default() {
        let result = Interpreter::resolve_function_interpreter(&[], None);
        assert_eq!(result, TranspilerInterpreter::default());
    }

    #[test]
    fn test_resolve_function_interpreter_shell_attribute() {
        let attrs = vec![Attribute::Shell(ShellType::Python)];
        let result = Interpreter::resolve_function_interpreter(&attrs, None);
        assert_eq!(result, TranspilerInterpreter::Python);
    }

    #[test]
    fn test_resolve_function_interpreter_shebang() {
        let result = Interpreter::resolve_function_interpreter(&[], Some("/usr/bin/env node"));
        assert_eq!(result, TranspilerInterpreter::Node);
    }

    #[test]
    fn test_resolve_function_interpreter_attribute_overrides_shebang() {
        let attrs = vec![Attribute::Shell(ShellType::Ruby)];
        let result =
            Interpreter::resolve_function_interpreter(&attrs, Some("/usr/bin/env python3"));
        // Attribute should take precedence
        assert_eq!(result, TranspilerInterpreter::Ruby);
    }

    #[test]
    fn test_execute_simple_function_call() {
        let mut interp = Interpreter::new();
        // Define and call a simple function
        let program = Program {
            statements: vec![
                Statement::SimpleFunctionDef {
                    name: "greet".to_string(),
                    params: vec![],
                    command_template: "echo hello".to_string(),
                    attributes: vec![],
                },
                Statement::FunctionCall {
                    name: "greet".to_string(),
                    args: vec![],
                },
            ],
        };
        // This calls shell commands, should work on any Unix system
        let result = interp.execute(program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_function_double_underscore_resolution() {
        let mut interp = Interpreter::new();
        interp
            .simple_functions
            .insert("nested:func".to_string(), "echo nested".to_string());
        interp.function_metadata.insert(
            "nested:func".to_string(),
            FunctionMetadata {
                attributes: vec![],
                shebang: None,
                params: vec![],
            },
        );

        // Should resolve nested__func -> nested:func
        let result = interp.call_function_without_parens("nested__func", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_function_underscore_to_colon_resolution() {
        let mut interp = Interpreter::new();
        interp
            .simple_functions
            .insert("docker:build".to_string(), "echo building".to_string());
        interp.function_metadata.insert(
            "docker:build".to_string(),
            FunctionMetadata {
                attributes: vec![],
                shebang: None,
                params: vec![],
            },
        );

        // Should resolve docker_build -> docker:build
        let result = interp.call_function_without_parens("docker_build", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_function_subcommand_resolution() {
        let mut interp = Interpreter::new();
        interp
            .simple_functions
            .insert("docker:shell".to_string(), "echo shell".to_string());
        interp.function_metadata.insert(
            "docker:shell".to_string(),
            FunctionMetadata {
                attributes: vec![],
                shebang: None,
                params: vec![],
            },
        );

        // Calling "docker" with arg "shell" should resolve to docker:shell
        let result = interp.call_function_without_parens("docker", &["shell".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_os_filtered_function() {
        let mut interp = Interpreter::new();
        // Windows function on Linux - should be skipped
        let program = Program {
            statements: vec![Statement::SimpleFunctionDef {
                name: "win_only".to_string(),
                params: vec![],
                command_template: "echo windows".to_string(),
                attributes: vec![Attribute::Os(crate::ast::OsPlatform::Windows)],
            }],
        };
        interp.execute(program).unwrap();

        if cfg!(not(target_os = "windows")) {
            // Function should not be registered
            assert!(!interp.get_simple_functions().contains_key("win_only"));
        }
    }

    #[test]
    fn test_get_simple_function_attributes() {
        let mut interp = Interpreter::new();
        interp
            .simple_functions
            .insert("test".to_string(), "echo test".to_string());
        interp.function_metadata.insert(
            "test".to_string(),
            FunctionMetadata {
                attributes: vec![Attribute::Shell(ShellType::Bash)],
                shebang: None,
                params: vec![],
            },
        );

        let attrs = interp.get_simple_function_attributes("test");
        assert_eq!(attrs.len(), 1);
        assert!(matches!(attrs[0], Attribute::Shell(ShellType::Bash)));

        // Non-existent function returns empty slice
        let attrs = interp.get_simple_function_attributes("nonexistent");
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_get_block_function_metadata() {
        let mut interp = Interpreter::new();
        interp.function_metadata.insert(
            "build".to_string(),
            FunctionMetadata {
                attributes: vec![Attribute::Shell(ShellType::Bash)],
                shebang: Some("#!/bin/bash".to_string()),
                params: vec![],
            },
        );

        let (attrs, shebang) = interp.get_block_function_metadata("build");
        assert_eq!(attrs.len(), 1);
        assert_eq!(shebang, Some("#!/bin/bash"));

        // Non-existent function
        let (attrs, shebang) = interp.get_block_function_metadata("nonexistent");
        assert!(attrs.is_empty());
        assert!(shebang.is_none());
    }
}
