// Transpiler for converting Runfile functions to shell syntax
use crate::ast::ShellType;

#[derive(Debug, Clone, PartialEq)]
pub enum Interpreter {
    Sh,
    Bash,
    Pwsh,
    Python,
    Python3,
    Node,
    Ruby,
}

impl Interpreter {
    /// Check if this interpreter is compatible with another for function composition
    #[must_use]
    pub fn is_compatible_with(&self, other: &Interpreter) -> bool {
        matches!(
            (self, other),
            (
                Interpreter::Sh | Interpreter::Bash,
                Interpreter::Sh | Interpreter::Bash
            ) | (Interpreter::Pwsh, Interpreter::Pwsh)
                | (
                    Interpreter::Python | Interpreter::Python3,
                    Interpreter::Python | Interpreter::Python3
                )
                | (Interpreter::Node, Interpreter::Node)
                | (Interpreter::Ruby, Interpreter::Ruby)
        )
    }

    /// Convert `ShellType` to Interpreter
    #[must_use]
    pub fn from_shell_type(shell_type: &ShellType) -> Self {
        match shell_type {
            ShellType::Sh => Interpreter::Sh,
            ShellType::Bash => Interpreter::Bash,
            ShellType::Pwsh => Interpreter::Pwsh,
            ShellType::Python => Interpreter::Python,
            ShellType::Python3 => Interpreter::Python3,
            ShellType::Node => Interpreter::Node,
            ShellType::Ruby => Interpreter::Ruby,
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        if cfg!(target_os = "windows") {
            Interpreter::Pwsh
        } else {
            Interpreter::Sh
        }
    }
}

/// Transpile a function to shell syntax (sh/bash)
///
/// # Arguments
/// * `name` - Function name (may contain colons)
/// * `body` - Function body (command template or block)
/// * `is_block` - Whether this is a block function
#[must_use]
pub fn transpile_to_shell(name: &str, body: &str, is_block: bool) -> String {
    let sanitised = sanitise_name(name);

    if is_block {
        // Block function - body already contains multiple lines
        let indented = indent(body, "    ");
        format!("{sanitised}() {{\n{indented}\n}}")
    } else {
        // Simple function - single command
        format!("{sanitised}() {{\n    {body}\n}}")
    }
}

/// Transpile a function to `PowerShell` syntax
#[must_use]
pub fn transpile_to_pwsh(name: &str, body: &str, is_block: bool) -> String {
    let sanitised = sanitise_name(name);

    if is_block {
        let indented = indent(body, "    ");
        format!("function {sanitised} {{\n{indented}\n}}")
    } else {
        format!("function {sanitised} {{\n    {body}\n}}")
    }
}

/// sanitise function name by replacing colons with double underscores
#[must_use]
pub fn sanitise_name(name: &str) -> String {
    name.replace(':', "__")
}

/// Indent each line of text by the given prefix
fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{prefix}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rewrite call sites in function body to use sanitised names
///
/// This replaces function names containing colons with their sanitised versions
/// (colons replaced with double underscores). Only replaces whole-word matches.
#[must_use]
pub fn rewrite_call_sites(body: &str, sibling_names: &[&str]) -> String {
    let mut result = body.to_string();

    for sibling in sibling_names {
        if sibling.contains(':') {
            let sanitised = sanitise_name(sibling);
            result = replace_word(&result, sibling, &sanitised);
        }
    }

    result
}

/// Replace whole-word occurrences of a pattern in text
///
/// This ensures we only replace actual function calls, not partial matches
/// within other words.
fn replace_word(text: &str, pattern: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    let pattern_chars: Vec<char> = pattern.chars().collect();

    'outer: while let Some(ch) = chars.next() {
        // Try to match pattern
        if ch == pattern_chars[0] {
            // Look ahead to match the rest of the pattern
            let mut matched = vec![ch];
            let mut peek_ahead: Vec<char> = Vec::new();

            for &pattern_ch in &pattern_chars[1..] {
                if let Some(&next_ch) = chars.peek() {
                    peek_ahead.push(next_ch);
                    if next_ch == pattern_ch {
                        // We know next() will return Some because peek() just returned Some
                        if let Some(consumed) = chars.next() {
                            matched.push(consumed);
                        }
                    } else {
                        // No match, output what we collected and continue
                        result.push_str(&matched.iter().collect::<String>());
                        continue 'outer;
                    }
                } else {
                    // End of string, no match
                    result.push_str(&matched.iter().collect::<String>());
                    continue 'outer;
                }
            }

            // We matched the pattern, now check word boundaries
            // Check if there's a non-word character (or start/end) before and after
            let before_ok =
                result.is_empty() || result.chars().last().is_none_or(|c| !is_word_char(c));
            let after_ok = chars.peek().is_none_or(|&c| !is_word_char(c));

            if before_ok && after_ok {
                // Valid word boundary, replace
                result.push_str(replacement);
            } else {
                // Not a word boundary, keep original
                result.push_str(&matched.iter().collect::<String>());
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Check if a character is a word character (alphanumeric, underscore, or colon)
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == ':'
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_transpile_simple_shell_function() {
        let result = transpile_to_shell("build", "cargo build", false);
        assert_eq!(result, "build() {\n    cargo build\n}");
    }

    #[test]
    fn test_transpile_block_shell_function() {
        let body = "cargo build --release\ncargo test";
        let result = transpile_to_shell("ci", body, true);
        assert_eq!(
            result,
            "ci() {\n    cargo build --release\n    cargo test\n}"
        );
    }

    #[test]
    fn test_transpile_colon_name() {
        let result = transpile_to_shell("docker:build", "docker build .", false);
        assert_eq!(result, "docker__build() {\n    docker build .\n}");
    }

    #[test]
    fn test_transpile_simple_pwsh_function() {
        let result = transpile_to_pwsh("build", "cargo build", false);
        assert_eq!(result, "function build {\n    cargo build\n}");
    }

    #[test]
    fn test_transpile_block_pwsh_function() {
        let body = "cargo build\ncargo test";
        let result = transpile_to_pwsh("ci", body, true);
        assert_eq!(result, "function ci {\n    cargo build\n    cargo test\n}");
    }

    #[test]
    fn test_sanitise_name() {
        assert_eq!(sanitise_name("docker:build"), "docker__build");
        assert_eq!(sanitise_name("simple"), "simple");
        assert_eq!(sanitise_name("multi:level:name"), "multi__level__name");
    }

    #[test]
    fn test_rewrite_call_sites_simple() {
        let body = "docker:build\ndocker:push";
        let siblings = vec!["docker:build", "docker:push"];
        let result = rewrite_call_sites(body, &siblings);
        assert_eq!(result, "docker__build\ndocker__push");
    }

    #[test]
    fn test_rewrite_call_sites_with_args() {
        let body = "docker:build --tag latest\ndocker:push myapp";
        let siblings = vec!["docker:build", "docker:push"];
        let result = rewrite_call_sites(body, &siblings);
        assert_eq!(result, "docker__build --tag latest\ndocker__push myapp");
    }

    #[test]
    fn test_rewrite_call_sites_no_partial_match() {
        let body = "docker:build\nmy_docker:build_script";
        let siblings = vec!["docker:build"];
        let result = rewrite_call_sites(body, &siblings);
        // Should only replace the exact match, not the partial one
        assert_eq!(result, "docker__build\nmy_docker:build_script");
    }

    #[test]
    fn test_rewrite_call_sites_no_colons() {
        let body = "build\ntest\ndeploy";
        let siblings = vec!["build", "test", "deploy"];
        let result = rewrite_call_sites(body, &siblings);
        // No changes when names don't have colons
        assert_eq!(result, "build\ntest\ndeploy");
    }

    #[test]
    fn test_interpreter_compatibility_sh_bash() {
        let sh = Interpreter::Sh;
        let bash = Interpreter::Bash;

        assert!(sh.is_compatible_with(&bash));
        assert!(bash.is_compatible_with(&sh));
        assert!(sh.is_compatible_with(&sh));
        assert!(bash.is_compatible_with(&bash));
    }

    #[test]
    fn test_interpreter_compatibility_pwsh() {
        let pwsh = Interpreter::Pwsh;
        let sh = Interpreter::Sh;

        assert!(pwsh.is_compatible_with(&pwsh));
        assert!(!pwsh.is_compatible_with(&sh));
        assert!(!sh.is_compatible_with(&pwsh));
    }

    #[test]
    fn test_interpreter_compatibility_polyglot() {
        let python = Interpreter::Python;
        let python3 = Interpreter::Python3;
        let node = Interpreter::Node;
        let ruby = Interpreter::Ruby;
        let sh = Interpreter::Sh;

        // Python and Python3 can compose with each other
        assert!(python.is_compatible_with(&python3));
        assert!(python3.is_compatible_with(&python));

        // But not with other polyglot languages
        assert!(!python.is_compatible_with(&node));
        assert!(!python.is_compatible_with(&ruby));
        assert!(!python.is_compatible_with(&sh));
        assert!(!node.is_compatible_with(&ruby));
        assert!(!node.is_compatible_with(&sh));
        assert!(!ruby.is_compatible_with(&sh));

        // But they can compose with themselves
        assert!(python.is_compatible_with(&python));
        assert!(python3.is_compatible_with(&python3));
        assert!(node.is_compatible_with(&node));
        assert!(ruby.is_compatible_with(&ruby));
    }

    #[test]
    fn test_indent() {
        let text = "line1\nline2\nline3";
        let result = indent(text, "    ");
        assert_eq!(result, "    line1\n    line2\n    line3");
    }

    #[test]
    fn test_indent_with_empty_lines() {
        let text = "line1\n\nline3";
        let result = indent(text, "    ");
        assert_eq!(result, "    line1\n\n    line3");
    }

    #[test]
    fn test_replace_word_boundaries() {
        // Test that we only replace whole words
        assert_eq!(
            replace_word("docker:build", "docker:build", "docker__build"),
            "docker__build"
        );
        assert_eq!(
            replace_word("call docker:build here", "docker:build", "docker__build"),
            "call docker__build here"
        );
        assert_eq!(
            replace_word("docker:build\ndocker:push", "docker:build", "docker__build"),
            "docker__build\ndocker:push"
        );
    }

    #[test]
    fn test_replace_word_no_partial_match() {
        // Should not replace if it's part of another word
        let text = "my_docker:build_func";
        let result = replace_word(text, "docker:build", "docker__build");
        assert_eq!(result, "my_docker:build_func");
    }
}
