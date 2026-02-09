//! Shebang parsing utilities
//!
//! Handles detection and extraction of shebangs from function bodies

/// Parse shebang from function body
/// Returns the shebang string if found on the first non-empty, non-comment line
/// Lines starting with # (but not #!) are skipped
pub(super) fn parse_shebang(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .find(|line| {
            // Find the first line that is either a shebang or not a comment
            !line.starts_with('#') || line.starts_with("#!")
        })
        .and_then(|line| {
            if line.starts_with("#!") {
                Some(line[2..].trim().to_string())
            } else {
                None
            }
        })
}
