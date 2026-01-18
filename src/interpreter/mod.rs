//! Interpreter to execute the AST
//!
//! This module provides the main interpreter for executing parsed Run scripts.
//! It handles function definitions, variable substitution, and command execution
//! with support for multiple interpreters (sh, bash, python, node, ruby, etc.)

mod execution;
mod preamble;
mod shell;

use crate::ast::{Attribute, Expression, Program, Statement};
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
    /// 1. Direct match: "docker_shell" with args
    /// 2. If args exist, try first arg as subcommand: "docker" + "shell" -> "docker:shell"
    /// 3. Try replacing underscores with colons: "docker_shell" -> "docker:shell"
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
            return self.execute_simple_function(function_name, &command_template, args, &attributes);
        }

        // Try direct match - block functions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            let (attributes, shebang) = self.get_block_function_metadata(function_name);
            return self.execute_block_commands(function_name, &commands, args, &attributes, shebang);
        }

        // If we have args, try treating the first arg as a subcommand
        if !args.is_empty() {
            let nested_name = format!("{}:{}", function_name, args[0]);
            if let Some(command_template) = self.simple_functions.get(&nested_name).cloned() {
                let attributes = self.get_simple_function_attributes(&nested_name).to_vec();
                return self.execute_simple_function(&nested_name, &command_template, &args[1..], &attributes);
            }
            if let Some(commands) = self.block_functions.get(&nested_name).cloned() {
                let (attributes, shebang) = self.get_block_function_metadata(&nested_name);
                return self.execute_block_commands(&nested_name, &commands, &args[1..], &attributes, shebang);
            }
        }

        // Try replacing underscores with colons
        let with_colons = function_name.replace("_", ":");
        if with_colons != function_name {
            if let Some(command_template) = self.simple_functions.get(&with_colons).cloned() {
                let attributes = self.get_simple_function_attributes(&with_colons).to_vec();
                return self.execute_simple_function(&with_colons, &command_template, args, &attributes);
            }
            if let Some(commands) = self.block_functions.get(&with_colons).cloned() {
                let (attributes, shebang) = self.get_block_function_metadata(&with_colons);
                return self.execute_block_commands(&with_colons, &commands, args, &attributes, shebang);
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
            return self.execute_simple_function(function_name, &command_template, args, &attributes);
        }

        // Check for block function definitions
        if let Some(commands) = self.block_functions.get(function_name).cloned() {
            let (attributes, shebang) = self.get_block_function_metadata(function_name);
            return self.execute_block_commands(function_name, &commands, args, &attributes, shebang);
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
            Statement::BlockFunctionDef { name, params, commands, attributes, shebang } => {
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
    fn resolve_function_interpreter(&self, _name: &str, attributes: &[Attribute], shebang: Option<&str>) -> TranspilerInterpreter {
        // Check for @shell attribute
        for attr in attributes {
            if let Attribute::Shell(shell_type) = attr {
                return TranspilerInterpreter::from_shell_type(shell_type);
            }
        }

        // Check for shebang
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
        &self,
        target_name: &str,
        command_template: &str,
        args: &[String],
        attributes: &[Attribute],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Determine the target interpreter
        let target_interpreter = self.resolve_function_interpreter(target_name, attributes, None);

        // Create closure for resolve_interpreter
        let resolve_interpreter = |name: &str, attrs: &[Attribute], shebang: Option<&str>| {
            self.resolve_function_interpreter(name, attrs, shebang)
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
        let sibling_names: Vec<&str> = rewritable_names.iter().map(|s| s.as_str()).collect();

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
        let combined_script = execution::build_combined_script(
            var_preamble,
            func_preamble,
            rewritten_body,
        );

        // Substitute args and execute
        let substituted = self.substitute_args(&combined_script, args);
        shell::execute_single_shell_invocation(&substituted, &target_interpreter)
    }

    fn execute_block_commands(
        &self,
        target_name: &str,
        commands: &[String],
        args: &[String],
        attributes: &[Attribute],
        shebang: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Determine the target interpreter
        let target_interpreter = self.resolve_function_interpreter(target_name, attributes, shebang);

        // Check if this is a polyglot language (Python, Node, Ruby)
        let is_polyglot = matches!(
            target_interpreter,
            TranspilerInterpreter::Python | TranspilerInterpreter::Python3 |
            TranspilerInterpreter::Node | TranspilerInterpreter::Ruby
        );

        let full_script = commands.join("\n");

        if is_polyglot {
            // For polyglot languages, execute without preamble
            let script = if shebang.is_some() {
                shell::strip_shebang(&full_script)
            } else {
                full_script
            };

            let substituted = self.substitute_args(&script, args);
            let exec_attributes = execution::prepare_polyglot_attributes(attributes, shebang);

            return shell::execute_command_with_args(&substituted, &exec_attributes, args);
        }

        // For shell-compatible languages, build preamble and compose
        let resolve_interpreter = |name: &str, attrs: &[Attribute], shebang: Option<&str>| {
            self.resolve_function_interpreter(name, attrs, shebang)
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
        let sibling_names: Vec<&str> = rewritable_names.iter().map(|s| s.as_str()).collect();

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
        let combined_script = execution::build_combined_script(
            var_preamble,
            func_preamble,
            rewritten_body,
        );

        // Substitute args and execute
        let substituted = self.substitute_args(&combined_script, args);
        shell::execute_single_shell_invocation(&substituted, &target_interpreter)
    }
}
