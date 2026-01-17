//! Input preprocessing utilities
//!
//! Handles preprocessing of input text before parsing

/// Preprocess input to join lines ending with a backslash
pub(super) fn preprocess_escaped_newlines(input: &str) -> String {
    let mut result = String::new();
    let lines = input.lines();
    let mut buffer = String::new();
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.ends_with('\\') {
            if let Some(stripped) = trimmed.strip_suffix('\\') {
                buffer.push_str(stripped);
            }
            buffer.push(' ');
        } else {
            buffer.push_str(trimmed);
            result.push_str(buffer.trim_end());
            result.push('\n');
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        result.push_str(buffer.trim_end());
        result.push('\n');
    }
    result
}
