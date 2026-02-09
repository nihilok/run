//! Block function parsing helpers

use crate::ast::Attribute;

/// Parse and dedent block content
pub(super) fn parse_block_content(block_str: &str) -> String {
    // Remove leading '{' and trailing '}' but DON'T trim - preserve indentation
    let content_str = block_str
        .strip_prefix('{')
        .unwrap_or(block_str)
        .strip_suffix('}')
        .unwrap_or(block_str);

    // Split by newlines to process line by line
    let all_lines: Vec<&str> = content_str.lines().collect();

    // Skip leading and trailing empty/whitespace-only lines
    let start = all_lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(0);
    let end = all_lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(all_lines.len(), |i| i + 1);
    let lines: Vec<&str> = if start < end {
        all_lines[start..end].to_vec()
    } else {
        vec![]
    };

    // Find the minimum indentation (excluding empty lines)
    let min_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    // Build dedented lines
    let dedented_lines: Vec<String> = lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else if line.len() > min_indent {
                line[min_indent..].to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    dedented_lines.join("\n")
}

/// Split block content into commands based on shell type
pub(super) fn split_block_commands(content: &str, attributes: &[Attribute]) -> Vec<String> {
    let trimmed_content = content.trim();

    // Check if this function has a custom shell attribute
    let has_custom_shell = attributes
        .iter()
        .any(|attr| matches!(attr, Attribute::Shell(_)));

    if has_custom_shell {
        // For custom shells (Python, Node, etc.), never split by semicolons
        vec![trimmed_content.to_string()]
    } else if !trimmed_content.contains('\n') && trimmed_content.contains(';') {
        // Single-line block with semicolons: split into separate commands
        trimmed_content
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        // Multi-line block - keep as single script
        vec![trimmed_content.to_string()]
    }
}
