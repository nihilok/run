//! Utility functions shared across modules

use crate::ast::{ArgType, Attribute, OsPlatform};

/// Convert `ArgType` to JSON schema type string
#[must_use]
pub fn arg_type_to_json_type(arg_type: &ArgType) -> &'static str {
    match arg_type {
        ArgType::String => "string",
        ArgType::Integer => "integer",
        ArgType::Float => "number",
        ArgType::Boolean => "boolean",
        ArgType::Object => "object",
    }
}

/// Check if function attributes match the current platform
///
/// Returns `true` if:
/// - No OS attributes are present (available on all platforms)
/// - At least one OS attribute matches the current platform
#[must_use]
pub fn matches_current_platform(attributes: &[Attribute]) -> bool {
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
    os_attrs
        .iter()
        .any(|platform| platform_matches_current(platform))
}

/// Check if a specific platform matches the current OS
fn platform_matches_current(platform: &OsPlatform) -> bool {
    match platform {
        OsPlatform::Windows => cfg!(target_os = "windows"),
        OsPlatform::Linux => cfg!(target_os = "linux"),
        OsPlatform::MacOS => cfg!(target_os = "macos"),
        OsPlatform::Unix => cfg!(unix), // Matches Linux or macOS
    }
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

                // read the delimiter word — bash allows alphanumerics,
                // underscores, hyphens, dots, and similar non-metacharacters.
                let start = i;
                while i < len && is_heredoc_delim_char(chars[i]) {
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

/// Characters allowed in an unquoted heredoc delimiter (broadly: anything
/// that is not whitespace or a shell metacharacter).
fn is_heredoc_delim_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '+' | '/' | '~' | '!' | '%' | '@')
}

/// Build a boolean mask indicating which lines are inside a heredoc.
///
/// A line is considered "inside a heredoc" if it is between a heredoc opening
/// marker (`<<DELIM`) and the corresponding closing delimiter. The opening
/// marker line itself is NOT masked (it is a normal command), but every
/// subsequent line up to and including the closing delimiter IS masked.
#[must_use]
pub fn build_heredoc_mask(lines: &[&str]) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];
    let mut heredoc_stack: Vec<String> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if let Some(delim) = heredoc_stack.last() {
            mask[idx] = true;
            let trimmed = line.trim_end();
            if trimmed == delim || trimmed.trim_start_matches('\t') == delim {
                heredoc_stack.pop();
            }
        } else {
            // Not inside a heredoc — check whether this line opens one.
            heredoc_stack.extend(extract_heredoc_delimiters(line));
        }
    }

    mask
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ast::Attribute;

    #[test]
    fn test_arg_type_to_json_type() {
        assert_eq!(arg_type_to_json_type(&ArgType::String), "string");
        assert_eq!(arg_type_to_json_type(&ArgType::Integer), "integer");
        assert_eq!(arg_type_to_json_type(&ArgType::Float), "number");
        assert_eq!(arg_type_to_json_type(&ArgType::Boolean), "boolean");
        assert_eq!(arg_type_to_json_type(&ArgType::Object), "object");
    }

    #[test]
    fn test_matches_current_platform_no_os_attrs() {
        let attributes = vec![Attribute::Desc("Test".to_string())];
        assert!(matches_current_platform(&attributes));
    }

    #[test]
    fn test_matches_current_platform_unix() {
        let attributes = vec![Attribute::Os(OsPlatform::Unix)];

        if cfg!(unix) {
            assert!(matches_current_platform(&attributes));
        } else {
            assert!(!matches_current_platform(&attributes));
        }
    }

    #[test]
    fn test_matches_current_platform_windows() {
        let attributes = vec![Attribute::Os(OsPlatform::Windows)];

        if cfg!(windows) {
            assert!(matches_current_platform(&attributes));
        } else {
            assert!(!matches_current_platform(&attributes));
        }
    }

    #[test]
    fn test_matches_current_platform_multiple_os() {
        // Test with multiple OS attributes (e.g., linux + macos)
        let attributes = vec![
            Attribute::Os(OsPlatform::Linux),
            Attribute::Os(OsPlatform::MacOS),
        ];

        if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
            assert!(matches_current_platform(&attributes));
        } else {
            assert!(!matches_current_platform(&attributes));
        }
    }

    // --- heredoc helpers ---

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
    fn test_extract_heredoc_case_sensitive() {
        // Bash heredoc delimiters are case-sensitive
        assert_eq!(extract_heredoc_delimiters("cat <<eof"), vec!["eof"]);
        assert_eq!(extract_heredoc_delimiters("cat <<Eof"), vec!["Eof"]);
        assert_eq!(extract_heredoc_delimiters("cat <<EOF"), vec!["EOF"]);
    }

    #[test]
    fn test_extract_heredoc_escaped_quotes_in_string() {
        // Escaped quotes inside a double-quoted string should not confuse the tracker
        let result: Vec<String> = extract_heredoc_delimiters(r#"echo "foo \"<<EOF\" bar""#);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_heredoc_with_hyphen_in_delimiter() {
        assert_eq!(
            extract_heredoc_delimiters("cat <<END-OF-FILE"),
            vec!["END-OF-FILE"]
        );
    }

    #[test]
    fn test_extract_heredoc_with_dot_in_delimiter() {
        assert_eq!(
            extract_heredoc_delimiters("cat <<DATA.TXT"),
            vec!["DATA.TXT"]
        );
    }

    #[test]
    fn test_build_heredoc_mask_basic() {
        let lines = vec!["cat <<EOF", "content", "EOF", "echo done"];
        let mask = build_heredoc_mask(&lines);
        assert_eq!(mask, vec![false, true, true, false]);
    }

    #[test]
    fn test_build_heredoc_mask_multiple() {
        let lines = vec!["cat <<A", "a", "A", "cat <<B", "b", "B"];
        let mask = build_heredoc_mask(&lines);
        assert_eq!(mask, vec![false, true, true, false, true, true]);
    }
}
