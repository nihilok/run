// Transpiler for converting Runfile functions to shell syntax
use crate::ast::ShellType;
use which::which;

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
    /// Check if this interpreter is compatible with another for function composition.
    ///
    /// Compatibility is asymmetric for `sh`/`bash`: bash is a superset of sh and can
    /// run sh code, but sh (e.g. dash on Ubuntu) cannot run bash-specific syntax.
    ///
    /// - `bash.is_compatible_with(sh)` → `true`
    /// - `sh.is_compatible_with(bash)` → `false`
    #[must_use]
    pub fn is_compatible_with(&self, other: &Interpreter) -> bool {
        matches!(
            (self, other),
            // bash is a superset: it can run both bash and sh code
            (Interpreter::Bash, Interpreter::Sh | Interpreter::Bash)
                // sh can only run sh code (not bash-specific syntax)
                | (Interpreter::Sh, Interpreter::Sh)
                | (Interpreter::Pwsh, Interpreter::Pwsh)
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
        } else if which("bash").is_ok() {
            Interpreter::Bash
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

/// sanitise function name by replacing colons with double underscores and hyphens with underscores
#[must_use]
pub fn sanitise_name(name: &str) -> String {
    name.replace(':', "__").replace('-', "_")
}

/// Indent each line of text by the given prefix, respecting heredoc boundaries.
///
/// Lines inside a heredoc (between a `<<DELIM` marker and the closing `DELIM`)
/// are **not** indented because:
/// 1. Adding whitespace would change the content delivered to the command.
/// 2. Indenting the closing delimiter would prevent the shell from recognising it.
fn indent(text: &str, prefix: &str) -> String {
    let mut result = Vec::new();
    let mut heredoc_stack: Vec<String> = Vec::new();

    for line in text.lines() {
        if let Some(delim) = heredoc_stack.last() {
            // Inside a heredoc — emit the line verbatim (no indentation).
            result.push(line.to_string());
            // Check whether this line closes the innermost heredoc.
            // For `<<DELIM` the delimiter must appear alone; for `<<-DELIM`
            // bash strips leading tabs, so we also try after stripping tabs.
            let trimmed = line.trim_end();
            if trimmed == delim || trimmed.trim_start_matches('\t') == delim {
                heredoc_stack.pop();
            }
        } else {
            // Normal line — apply indentation.
            if line.trim().is_empty() {
                result.push(String::new());
            } else {
                result.push(format!("{prefix}{line}"));
            }
            // After emitting, check whether this line opens one or more heredocs.
            heredoc_stack.extend(extract_heredoc_delimiters(line));
        }
    }

    result.join("\n")
}

/// Extract heredoc delimiter names from a line of shell code.
///
/// Recognises `<<DELIM`, `<<-DELIM`, `<<"DELIM"`, `<<'DELIM'`, and `<<\DELIM`.
/// Skips `<<<` (here-strings) and `<<` that appears inside quoted strings.
#[must_use]
pub fn extract_heredoc_delimiters(line: &str) -> Vec<String> {
    let mut delimiters = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < len {
        match chars[i] {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '\\' if !in_single_quote && i + 1 < len => {
                i += 2;
                continue;
            }
            '<' if !in_single_quote
                && !in_double_quote
                && i + 1 < len
                && chars[i + 1] == '<'
                && !(i + 2 < len && chars[i + 2] == '<')
                && (i == 0 || chars[i - 1] != '<') =>
            {
                i += 2; // skip `<<`

                // optional dash (`<<-`)
                if i < len && chars[i] == '-' {
                    i += 1;
                }
                // optional whitespace
                while i < len && (chars[i] == ' ' || chars[i] == '\t') {
                    i += 1;
                }
                if i >= len {
                    break;
                }

                // optional quoting character: ', ", or backslash
                let close_quote = match chars[i] {
                    q @ ('\'' | '"') => {
                        i += 1;
                        Some(q)
                    }
                    '\\' => {
                        i += 1;
                        None
                    }
                    _ => None,
                };

                // read the delimiter word
                let start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }

                if i > start {
                    let delim: String = chars[start..i].iter().collect();
                    // skip the matching closing quote if present
                    if let Some(q) = close_quote
                        && i < len
                        && chars[i] == q
                    {
                        i += 1;
                    }
                    delimiters.push(delim);
                }
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    delimiters
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
        assert_eq!(sanitise_name("my-func"), "my_func");
        assert_eq!(sanitise_name("build-and-deploy"), "build_and_deploy");
        assert_eq!(sanitise_name("ns:my-func"), "ns__my_func");
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

        // bash is a superset of sh: it can run sh code
        assert!(bash.is_compatible_with(&sh));
        // sh cannot run bash-specific syntax
        assert!(!sh.is_compatible_with(&bash));
        // same-interpreter pairs are always compatible
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
    fn test_indent_skips_heredoc_content() {
        let text = "cat > file <<EOF\nKEY=value\nEOF\necho done";
        let result = indent(text, "    ");
        assert_eq!(
            result,
            "    cat > file <<EOF\nKEY=value\nEOF\n    echo done"
        );
    }

    #[test]
    fn test_indent_skips_heredoc_dash() {
        let text = "cat > file <<-END\n\tline1\n\tEND\necho done";
        let result = indent(text, "    ");
        assert_eq!(
            result,
            "    cat > file <<-END\n\tline1\n\tEND\n    echo done"
        );
    }

    #[test]
    fn test_indent_skips_quoted_heredoc_delimiter() {
        let text = "cat <<'MARKER'\n$NOT_EXPANDED\nMARKER";
        let result = indent(text, "    ");
        assert_eq!(result, "    cat <<'MARKER'\n$NOT_EXPANDED\nMARKER");
    }

    #[test]
    fn test_indent_multiple_heredocs() {
        let text = "cat <<A\na\nA\ncat <<B\nb\nB\necho end";
        let result = indent(text, "  ");
        assert_eq!(result, "  cat <<A\na\nA\n  cat <<B\nb\nB\n  echo end");
    }

    #[test]
    fn test_indent_ignores_heredoc_marker_in_quotes() {
        // <<EOF inside a quoted string should NOT trigger heredoc detection
        let text = "echo \"<<EOF is not a heredoc\"\necho normal";
        let result = indent(text, "    ");
        assert_eq!(
            result,
            "    echo \"<<EOF is not a heredoc\"\n    echo normal"
        );
    }

    #[test]
    fn test_extract_heredoc_delimiters_basic() {
        assert_eq!(extract_heredoc_delimiters("cat <<EOF"), vec!["EOF"]);
        assert_eq!(extract_heredoc_delimiters("cat <<-EOF"), vec!["EOF"]);
        assert_eq!(extract_heredoc_delimiters("cat <<'EOF'"), vec!["EOF"]);
        assert_eq!(extract_heredoc_delimiters("cat <<\"EOF\""), vec!["EOF"]);
    }

    #[test]
    fn test_extract_heredoc_skips_herestring() {
        let result: Vec<String> = extract_heredoc_delimiters("cat <<<EOF");
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_heredoc_inside_quotes() {
        let result: Vec<String> = extract_heredoc_delimiters("echo \"<<EOF\" more");
        assert!(result.is_empty());
    }

    #[test]
    fn test_transpile_block_with_heredoc() {
        let body = "cat > file <<EOF\nKEY=value\nEOF";
        let result = transpile_to_shell("setup", body, true);
        assert_eq!(result, "setup() {\n    cat > file <<EOF\nKEY=value\nEOF\n}");
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
