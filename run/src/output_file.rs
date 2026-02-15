//! Output file handling for MCP mode
//!
//! When running in MCP mode, command outputs exceeding a character budget (~300 tokens)
//! are written to files and the agent receives a truncated tail view with file path
//! and byte count information.
//!
//! Output files are placed under a `.run-output` directory in the project (Runfile) directory
//! when available, or in the system temp directory as a fallback.

use crate::config;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Character budget targeting ~300 tokens at ~4 chars/token
const OUTPUT_TRUNCATE_CHARS: usize = 1200;
static OUTPUT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Result of processing command output for MCP mode
#[derive(Debug)]
pub struct ProcessedOutput {
    /// The output to display (may be truncated with metadata)
    pub display_output: String,
    /// Optional path to the full output file
    pub file_path: Option<PathBuf>,
    /// Total byte count of the output
    pub total_bytes: usize,
}

/// Process command output for MCP mode.
/// If output is longer than the truncate threshold, writes to file and returns truncated output
/// with file path and line count. Otherwise returns original output.
///
/// # Errors
///
/// Returns `Err` if:
/// - System time cannot be determined
/// - The output directory cannot be created
/// - The output file cannot be written
pub fn process_output_for_mcp(
    output: &str,
    stream_label: &str,
) -> Result<ProcessedOutput, Box<dyn std::error::Error>> {
    let total_bytes = output.len();

    // If output is within the character budget, return as-is
    if total_bytes <= OUTPUT_TRUNCATE_CHARS {
        return Ok(ProcessedOutput {
            display_output: output.to_string(),
            file_path: None,
            total_bytes,
        });
    }

    // Generate unique filename based on timestamp and a monotonic counter to avoid collisions
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let sequence = OUTPUT_SEQ.fetch_add(1, Ordering::Relaxed);
    let output_dir = config::get_mcp_output_dir();

    // Ensure output directory exists
    fs::create_dir_all(&output_dir)?;

    // Include stream label and sequence to avoid collisions between streams and rapid successive commands
    let file_path = output_dir.join(format!(
        "run-output-{timestamp}-{stream_label}-{sequence}.txt"
    ));

    // Write full output to file
    let mut file = fs::File::create(&file_path)?;
    file.write_all(output.as_bytes())?;

    // Extract tail content: walk backwards through lines, accumulating bytes
    let lines: Vec<&str> = output.lines().collect();
    let mut accumulated = 0usize;
    let mut tail_start = lines.len();

    for (i, line) in lines.iter().enumerate().rev() {
        let line_cost = line.len() + 1; // +1 for newline separator
        if accumulated + line_cost > OUTPUT_TRUNCATE_CHARS {
            break;
        }
        accumulated += line_cost;
        tail_start = i;
    }

    let (truncated_output, shown_bytes) = if tail_start < lines.len() {
        // We got at least one complete line
        let tail = lines[tail_start..].join("\n");
        let bytes = tail.len();
        (tail, bytes)
    } else {
        // Single last line exceeds the budget — take the last OUTPUT_TRUNCATE_CHARS bytes
        let last = lines.last().unwrap_or(&"");
        let start = last.len().saturating_sub(OUTPUT_TRUNCATE_CHARS);
        let snippet = &last[start..];
        let tail = format!("...{snippet}");
        let bytes = tail.len();
        (tail, bytes)
    };

    let display_output = format!(
        "[Output truncated: {total_bytes} bytes, showing last {shown_bytes} bytes]\n\
         [Full output saved to: {}]\n\n\
         {truncated_output}",
        file_path.display(),
    );

    Ok(ProcessedOutput {
        display_output,
        file_path: Some(file_path),
        total_bytes,
    })
}

/// Check if MCP output directory is configured (indicating MCP mode)
#[must_use]
pub fn is_mcp_output_enabled() -> bool {
    crate::config::is_mcp_output_configured()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_short_output_unchanged() {
        let output = "line1\nline2\nline3";
        let result = process_output_for_mcp(output, "stdout").expect("Processing should succeed");

        assert_eq!(result.display_output, output);
        assert_eq!(result.total_bytes, output.len());
        assert!(result.file_path.is_none());
    }

    #[test]
    fn test_exactly_threshold_unchanged() {
        // Build a string exactly OUTPUT_TRUNCATE_CHARS bytes long
        let output = "x".repeat(OUTPUT_TRUNCATE_CHARS);
        let result = process_output_for_mcp(&output, "stdout").expect("Processing should succeed");

        assert_eq!(result.display_output, output);
        assert_eq!(result.total_bytes, OUTPUT_TRUNCATE_CHARS);
        assert!(result.file_path.is_none());
    }

    #[test]
    fn test_long_output_truncated() {
        // Create many short lines that together exceed the budget
        let lines: Vec<String> = (1..=500).map(|i| format!("line{i}")).collect();
        let output = lines.join("\n");
        assert!(output.len() > OUTPUT_TRUNCATE_CHARS);

        let result = process_output_for_mcp(&output, "stdout").expect("Processing should succeed");

        assert_eq!(result.total_bytes, output.len());
        assert!(result.file_path.is_some());
        assert!(
            result
                .display_output
                .contains(&format!("{} bytes", output.len()))
        );
        assert!(result.display_output.contains("showing last"));

        // The tail should contain the very last line
        let tail_part = result
            .display_output
            .split("\n\n")
            .last()
            .expect("Should have tail part");
        assert!(tail_part.contains("line500"));

        // Early lines should not appear in the tail
        assert!(!tail_part.contains("line1\n"));
    }

    #[test]
    fn test_single_long_line_truncated() {
        // A single line of 5000 chars exceeds the budget
        let output = "A".repeat(5000);
        assert!(output.len() > OUTPUT_TRUNCATE_CHARS);

        let result = process_output_for_mcp(&output, "stdout").expect("Processing should succeed");

        assert_eq!(result.total_bytes, 5000);
        assert!(result.file_path.is_some());
        assert!(result.display_output.contains("5000 bytes"));

        // The tail should start with "..." since the single line was truncated
        let tail_part = result
            .display_output
            .split("\n\n")
            .last()
            .expect("Should have tail part");
        assert!(tail_part.starts_with("..."));
        // The shown portion should be around OUTPUT_TRUNCATE_CHARS + 3 ("..." prefix)
        assert!(tail_part.len() <= OUTPUT_TRUNCATE_CHARS + 4);
    }
}
