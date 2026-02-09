//! Utility functions shared across modules

use crate::ast::{ArgType, Attribute, OsPlatform};

/// Convert `ArgType` to JSON schema type string
#[must_use]
pub fn arg_type_to_json_type(arg_type: &ArgType) -> &'static str {
    match arg_type {
        ArgType::String => "string",
        ArgType::Integer => "integer",
        ArgType::Boolean => "boolean",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Attribute;

    #[test]
    fn test_arg_type_to_json_type() {
        assert_eq!(arg_type_to_json_type(&ArgType::String), "string");
        assert_eq!(arg_type_to_json_type(&ArgType::Integer), "integer");
        assert_eq!(arg_type_to_json_type(&ArgType::Boolean), "boolean");
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
}
