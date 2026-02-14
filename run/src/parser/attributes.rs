//! Attribute parsing for function annotations
//!
//! Handles parsing of @ directives like @os, @shell, @desc, and @arg

use crate::ast::{ArgMetadata, ArgType, Attribute, OsPlatform, ShellType};

/// Parse attributes from lines of the original input
pub(super) fn parse_attributes_from_lines(input: &str, line_num: usize) -> Vec<Attribute> {
    let mut attributes = Vec::new();
    let lines: Vec<&str> = input.lines().collect();

    if line_num == 0 {
        return attributes;
    }

    // Look backward from the function definition line to collect attributes
    let mut i = line_num - 1;
    loop {
        // Check if index is valid
        if i >= lines.len() {
            break;
        }

        let line = lines[i].trim();

        // If we hit an empty line or a non-comment line, stop
        if line.is_empty() || (!line.starts_with('#')) {
            break;
        }

        // If it's an attribute comment, parse it
        if line.starts_with("# @") || line.starts_with("#@") {
            if let Some(attr) = parse_attribute_line(line) {
                attributes.push(attr);
            }
        } else if line.starts_with('#') {
            // Regular comment - continue looking backward
        } else {
            break;
        }

        if i == 0 {
            break;
        }
        i -= 1;
    }

    // Reverse since we collected them backward
    attributes.reverse();
    attributes
}

/// Strip surrounding quotes from a string
fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if ((trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
        && trimmed.len() >= 2
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

/// Parse a single attribute line
fn parse_attribute_line(line: &str) -> Option<Attribute> {
    // Parse "# @os <platform>" or "# @shell <shell>" or "# @desc <text>" or "# @arg <spec>"
    let line = line.trim();

    // Remove "# " or "#" prefix and "@" symbol
    let without_hash = line
        .strip_prefix("# @")
        .or_else(|| line.strip_prefix("#@"))?;

    // Handle @desc - everything after "@desc " is the description
    if let Some(desc_text) = without_hash.strip_prefix("desc ") {
        return Some(Attribute::Desc(strip_quotes(desc_text)));
    }

    // Handle @arg - format: "1:name type description"
    if let Some(arg_text) = without_hash.strip_prefix("arg ") {
        return parse_arg_attribute(arg_text);
    }

    let parts: Vec<&str> = without_hash.split_whitespace().collect();

    if parts.len() < 2 {
        return None;
    }

    match parts[0] {
        "os" => {
            let platform = match parts[1] {
                "windows" => OsPlatform::Windows,
                "linux" => OsPlatform::Linux,
                "macos" => OsPlatform::MacOS,
                "unix" => OsPlatform::Unix,
                _ => return None,
            };
            Some(Attribute::Os(platform))
        }
        "shell" => {
            let shell = match parts[1] {
                "python" => ShellType::Python,
                "python3" => ShellType::Python3,
                "node" => ShellType::Node,
                "ruby" => ShellType::Ruby,
                "pwsh" => ShellType::Pwsh,
                "bash" => ShellType::Bash,
                "sh" => ShellType::Sh,
                _ => return None,
            };
            Some(Attribute::Shell(shell))
        }
        _ => None,
    }
}

/// Parse an @arg attribute specification
fn parse_arg_attribute(arg_text: &str) -> Option<Attribute> {
    // Format:
    // - "1:name type description" (old style with position)
    // - "name description" (new hybrid style without position)
    let arg_text = arg_text.trim();

    // Check if it has a position prefix (number followed by colon)
    // The prefix must be entirely numeric
    let has_position = if let Some(colon_pos) = arg_text.find(':') {
        let prefix = &arg_text[..colon_pos];
        !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    };

    if has_position {
        // Old style: "1:name type description"
        let colon_pos = arg_text.find(':')?;
        let position_str = &arg_text[..colon_pos];
        let rest = &arg_text[colon_pos + 1..];

        let position: usize = position_str.parse().ok()?;

        // Split rest by whitespace
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let name = parts[0].to_string();

        // Check if second part is a type
        let (arg_type, desc_start_idx) = if parts.len() > 1 {
            match parts[1] {
                "string" => (ArgType::String, 2),
                "integer" => (ArgType::Integer, 2),
                "float" | "number" => (ArgType::Float, 2),
                "boolean" => (ArgType::Boolean, 2),
                "object" | "dict" => (ArgType::Object, 2),
                _ => (ArgType::String, 1), // Default to string, description starts at index 1
            }
        } else {
            (ArgType::String, 1)
        };

        // Join remaining parts as description
        let description = if desc_start_idx < parts.len() {
            strip_quotes(&parts[desc_start_idx..].join(" "))
        } else {
            String::new()
        };

        Some(Attribute::Arg(ArgMetadata {
            position,
            name,
            arg_type,
            description,
        }))
    } else {
        // New hybrid style: "name description"
        let parts: Vec<&str> = arg_text.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let name = parts[0].to_string();

        // Rest is description
        let description = if parts.len() > 1 {
            strip_quotes(&parts[1..].join(" "))
        } else {
            String::new()
        };

        // For hybrid style, use position 0 as a marker (won't be used anyway)
        Some(Attribute::Arg(ArgMetadata {
            position: 0,
            name,
            arg_type: ArgType::String,
            description,
        }))
    }
}
