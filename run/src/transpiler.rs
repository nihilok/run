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
/// (colons replaced with double underscores). Only replaces names in command
/// position — at the start of a line (after optional whitespace) or after shell
/// command separators (`&&`, `||`, `;`, `|`, `(`). Names appearing as arguments
/// to other commands (e.g. `pnpm test:unit`) are left untouched.
#[must_use]
pub fn rewrite_call_sites(body: &str, sibling_names: &[&str]) -> String {
    let colon_siblings: Vec<&str> = sibling_names
        .iter()
        .filter(|s| s.contains(':'))
        .copied()
        .collect();

    if colon_siblings.is_empty() {
        return body.to_string();
    }

    body.lines()
        .map(|line| rewrite_line(line, &colon_siblings))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rewrite sibling call sites within a single line.
///
/// Scans for command positions (start of line, after `&&`, `||`, `;`, `|`, `(`)
/// and only rewrites sibling names found there.
fn rewrite_line(line: &str, colon_siblings: &[&str]) -> String {
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Detect command position: start of line or just after a separator
        if is_command_position(&result) {
            // Skip whitespace
            let ws_start = i;
            while i < len && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }

            // Try to match a sibling name at this position
            if let Some((sibling, end)) = match_sibling_at(&chars, i, colon_siblings) {
                // Check word boundary after the match
                let after_ok = end >= len || !is_word_char(chars[end]);
                if after_ok {
                    // Push the whitespace, then the sanitised name
                    result.push_str(&chars[ws_start..i].iter().collect::<String>());
                    result.push_str(&sanitise_name(sibling));
                    i = end;
                    continue;
                }
            }

            // No match — rewind to before whitespace skip and fall through
            i = ws_start;
        }

        // Check for command separators that introduce a new command position
        if i < len {
            // Check for two-char separators: && ||
            if i + 1 < len {
                let two = [chars[i], chars[i + 1]];
                if two == ['&', '&'] || two == ['|', '|'] {
                    result.push(chars[i]);
                    result.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
            }
            // Single-char separators: ; | (
            if chars[i] == ';' || chars[i] == '|' || chars[i] == '(' {
                result.push(chars[i]);
                i += 1;
                continue;
            }

            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Check whether `result` (the output so far for the current line) indicates
/// that the next token would be in command position.
fn is_command_position(result: &str) -> bool {
    // Empty → start of line
    if result.is_empty() {
        return true;
    }
    // Check the last non-whitespace character
    let trimmed = result.trim_end();
    if trimmed.is_empty() {
        return true;
    }
    let Some(last) = trimmed.chars().last() else {
        return true;
    };
    // After these characters the next token is a new command
    matches!(last, '&' | '|' | ';' | '(')
}

/// Try to match any sibling name at position `start` in `chars`.
/// Returns the matched sibling name and the end index if found.
fn match_sibling_at<'a>(
    chars: &[char],
    start: usize,
    colon_siblings: &[&'a str],
) -> Option<(&'a str, usize)> {
    // Try longest match first to avoid prefix conflicts
    let mut best: Option<(&'a str, usize)> = None;

    for &sibling in colon_siblings {
        let sib_chars: Vec<char> = sibling.chars().collect();
        let end = start + sib_chars.len();

        if end > chars.len() {
            continue;
        }

        if chars[start..end]
            .iter()
            .zip(sib_chars.iter())
            .all(|(a, b)| a == b)
        {
            // Check that character before is not a word char (for word boundary)
            let before_ok = start == 0 || !is_word_char(chars[start - 1]);
            if before_ok && best.is_none_or(|(prev, _)| sibling.len() > prev.len()) {
                best = Some((sibling, end));
            }
        }
    }

    best
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
    fn test_rewrite_word_boundaries() {
        // Whole-word match in command position → rewrite
        let result = rewrite_call_sites("docker:build", &["docker:build"]);
        assert_eq!(result, "docker__build");

        // In command position on each line → rewrite
        let result = rewrite_call_sites(
            "docker:build\ndocker:push",
            &["docker:build", "docker:push"],
        );
        assert_eq!(result, "docker__build\ndocker__push");
    }

    #[test]
    fn test_rewrite_no_partial_match() {
        // Should not replace if it's part of another word
        let result = rewrite_call_sites("my_docker:build_func", &["docker:build"]);
        assert_eq!(result, "my_docker:build_func");
    }

    #[test]
    fn test_rewrite_call_sites_should_not_rewrite_arguments() {
        // When a sibling name appears as an argument to another command (not in
        // command position), it should NOT be rewritten. This is the bug that
        // causes `pnpm test:unit` to become `pnpm test__unit`.
        let body = "pnpm test:unit \"$@\"";
        let siblings = vec!["test:unit"];
        let result = rewrite_call_sites(body, &siblings);
        assert_eq!(result, "pnpm test:unit \"$@\"");
    }

    #[test]
    fn test_rewrite_call_sites_command_position_vs_argument() {
        // test:unit at the start of a line IS a function call → rewrite
        // test:unit as an argument to pnpm is NOT a function call → don't rewrite
        let body = "test:unit\npnpm test:unit \"$@\"";
        let siblings = vec!["test:unit"];
        let result = rewrite_call_sites(body, &siblings);
        assert_eq!(result, "test__unit\npnpm test:unit \"$@\"");
    }

    #[test]
    fn test_rewrite_call_sites_sibling_name_as_argument_to_echo() {
        // echo test:build should not rewrite the argument
        let body = "echo \"running test:build\"\ntest:build";
        let siblings = vec!["test:build"];
        let result = rewrite_call_sites(body, &siblings);
        assert_eq!(result, "echo \"running test:build\"\ntest__build");
    }

    #[test]
    fn test_rewrite_call_sites_pnpm_wrapper_pattern() {
        // Real-world pattern: a task wraps pnpm and the aggregate task calls siblings.
        // The preamble function bodies should keep `pnpm test:unit` intact,
        // while the aggregate body should rewrite direct calls.
        let unit_body = "pnpm test:unit \"$@\"";
        let integration_body = "pnpm test:integration \"$@\"";
        let all_body = "test:unit\ntest:integration";
        let siblings = vec!["test:unit", "test:integration"];

        // The aggregate task's body: direct calls should be rewritten
        let result = rewrite_call_sites(all_body, &siblings);
        assert_eq!(result, "test__unit\ntest__integration");

        // The sibling bodies (used in preamble): args should NOT be rewritten
        let result = rewrite_call_sites(unit_body, &siblings);
        assert_eq!(result, "pnpm test:unit \"$@\"");

        let result = rewrite_call_sites(integration_body, &siblings);
        assert_eq!(result, "pnpm test:integration \"$@\"");
    }

    #[test]
    fn test_rewrite_call_sites_after_command_separators() {
        // Sibling names after && || ; | ( should be rewritten (command position)
        let siblings = vec!["test:unit", "test:lint"];

        assert_eq!(
            rewrite_call_sites("test:unit && test:lint", &siblings),
            "test__unit && test__lint"
        );
        assert_eq!(
            rewrite_call_sites("test:unit || test:lint", &siblings),
            "test__unit || test__lint"
        );
        assert_eq!(
            rewrite_call_sites("test:unit; test:lint", &siblings),
            "test__unit; test__lint"
        );
        assert_eq!(rewrite_call_sites("(test:unit)", &siblings), "(test__unit)");
    }

    #[test]
    fn test_rewrite_call_sites_indented() {
        // Indented calls (e.g. inside if/then blocks) should still be rewritten
        let body = "    test:unit\n    test:lint";
        let siblings = vec!["test:unit", "test:lint"];
        let result = rewrite_call_sites(body, &siblings);
        assert_eq!(result, "    test__unit\n    test__lint");
    }
}
