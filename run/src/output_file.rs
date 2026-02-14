//! Output file handling for MCP mode
//!
//! When running in MCP mode, command outputs longer than 50 lines are written to
//! files and the agent receives a truncated view with file path and line count information.
//!
//! Output files are placed under a `.run-output` directory in the project (Runfile) directory
//! when available, or in the system temp directory as a fallback.

use crate::config;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// Higher threshold to reduce needless truncation while still protecting MCP payload size
const OUTPUT_TRUNCATE_LINES: usize = 32;
static OUTPUT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Result of processing command output for MCP mode
#[derive(Debug)]
pub struct ProcessedOutput {
    /// The output to display (may be truncated with metadata)
    pub display_output: String,
    /// Optional path to the full output file
    pub file_path: Option<PathBuf>,
    /// Total line count in the output
    pub total_lines: usize,
}

/// Process command output for MCP mode
/// If output is longer than the truncate threshold, writes to file and returns truncated output
/// with file path and line count. Otherwise returns original output.
pub fn process_output_for_mcp(
    output: &str,
    stream_label: &str,
) -> Result<ProcessedOutput, Box<dyn std::error::Error>> {
    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    // If output is within the threshold, return as-is
    if total_lines <= OUTPUT_TRUNCATE_LINES {
        return Ok(ProcessedOutput {
            display_output: output.to_string(),
            file_path: None,
            total_lines,
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

    // Create truncated output with metadata
    let tail_lines: Vec<&str> = lines
        .iter()
        .rev()
        .take(OUTPUT_TRUNCATE_LINES)
        .rev()
        .copied()
        .collect();
    let truncated_output = tail_lines.join("\n");

    let display_output = format!(
        "[Output truncated: {} total lines, showing last {}]\n[Full output saved to: {}]\n\n{}",
        total_lines,
        OUTPUT_TRUNCATE_LINES,
        file_path.display(),
        truncated_output
    );

    Ok(ProcessedOutput {
        display_output,
        file_path: Some(file_path),
        total_lines,
    })
}

/// Check if MCP output directory is configured (indicating MCP mode)
#[must_use]
pub fn is_mcp_output_enabled() -> bool {
    crate::config::is_mcp_output_configured()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_short_output_unchanged() {
        let output = "line1\nline2\nline3";
        let result = process_output_for_mcp(output, "stdout").expect("Processing should succeed");

        assert_eq!(result.display_output, output);
        assert_eq!(result.total_lines, 3);
        assert!(result.file_path.is_none());
    }

    #[test]
    fn test_exactly_threshold_lines_unchanged() {
        let output = (1..=OUTPUT_TRUNCATE_LINES)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let result = process_output_for_mcp(&output, "stdout").expect("Processing should succeed");

        assert_eq!(result.display_output, output);
        assert_eq!(result.total_lines, OUTPUT_TRUNCATE_LINES);
        assert!(result.file_path.is_none());
    }

    #[test]
    fn test_long_output_truncated() {
        let lines: Vec<String> = (1..=OUTPUT_TRUNCATE_LINES + 5)
            .map(|i| format!("line{i}"))
            .collect();
        let output = lines.join("\n");

        let result = process_output_for_mcp(&output, "stdout").expect("Processing should succeed");

        assert_eq!(result.total_lines, OUTPUT_TRUNCATE_LINES + 5);
        assert!(result.file_path.is_some());
        assert!(result.display_output.contains(&format!(
            "Output truncated: {} total lines",
            OUTPUT_TRUNCATE_LINES + 5
        )));
        assert!(
            result
                .display_output
                .contains(&format!("showing last {OUTPUT_TRUNCATE_LINES}"))
        );

        // Check that the tail contains the last OUTPUT_TRUNCATE_LINES lines
        let tail_part = result
            .display_output
            .split("\n\n")
            .last()
            .expect("Should have tail part");
        let first_tail_line = OUTPUT_TRUNCATE_LINES + 5 - OUTPUT_TRUNCATE_LINES + 1;
        assert!(tail_part.contains(&format!("line{first_tail_line}"))); // First line of tail
        assert!(tail_part.contains(&format!("line{}", OUTPUT_TRUNCATE_LINES + 5))); // Last line of tail

        // Ensure early lines are not in the actual output tail
        let lines_only: Vec<&str> = tail_part.lines().collect();
        assert!(
            !lines_only
                .iter()
                .any(|line| line.contains("line1\n") || *line == "line1")
        );
    }
}
