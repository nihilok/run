//! Input preprocessing utilities
//!
//! Handles preprocessing of input text before parsing and maps preprocessed
//! byte offsets back to the original un-preprocessed input.

/// A contiguous segment mapping a byte range in the preprocessed string back
/// to a byte range in the original input.
///
/// Synthetic characters (such as the space replacing `\<newline>` or inserted
/// newlines) use a zero-length original range (`orig_start == orig_end`)
/// anchored to the corresponding offset in the original input.
#[derive(Debug, Clone, Copy)]
struct SourceSegment {
    prep_start: usize,
    prep_end: usize,
    orig_start: usize,
    orig_end: usize,
}

/// Mapping from byte offsets in preprocessed input back to original input.
#[derive(Debug, Clone)]
pub(super) struct SourceMap {
    segments: Vec<SourceSegment>,
    original_len: usize,
}

impl SourceMap {
    /// Map a byte offset in the preprocessed string back to a byte offset
    /// in the original input string.
    pub(super) fn map_offset(&self, prep_offset: usize) -> usize {
        let idx = self.segments.partition_point(|s| s.prep_end <= prep_offset);
        if let Some(seg) = self.segments.get(idx)
            && prep_offset >= seg.prep_start
        {
            if seg.orig_start == seg.orig_end {
                return seg.orig_start;
            }
            return seg.orig_start + (prep_offset - seg.prep_start);
        }
        self.original_len
    }
}

fn flush_buffer(
    result: &mut String,
    segments: &mut Vec<SourceSegment>,
    buffer: &mut String,
    buf_segments: &mut Vec<SourceSegment>,
) {
    let trimmed_len = buffer.trim_end().len();
    let result_start = result.len();

    for seg in buf_segments.drain(..) {
        if seg.prep_start >= trimmed_len {
            continue;
        }
        let clamped_end = seg.prep_end.min(trimmed_len);
        let orig_end = if seg.orig_start == seg.orig_end {
            seg.orig_start
        } else {
            seg.orig_start + (clamped_end - seg.prep_start)
        };
        segments.push(SourceSegment {
            prep_start: result_start + seg.prep_start,
            prep_end: result_start + clamped_end,
            orig_start: seg.orig_start,
            orig_end,
        });
    }

    result.push_str(&buffer[..trimmed_len]);
    buffer.clear();
}

/// Preprocess input to join lines ending with a backslash, returning both the
/// preprocessed string and a `SourceMap` from preprocessed offsets to original offsets.
pub(super) fn preprocess_escaped_newlines_with_map(input: &str) -> (String, SourceMap) {
    let mut result = String::new();
    let mut segments = Vec::new();
    let mut buffer = String::new();
    let mut buf_segments = Vec::new();
    let mut pos: usize = 0;

    for line in input.lines() {
        let line_start = pos;
        pos += line.len();
        if input.as_bytes().get(pos..pos + 2) == Some(b"\r\n") {
            pos += 2;
        } else if input.as_bytes().get(pos) == Some(&b'\n') {
            pos += 1;
        }

        let trimmed = line.trim_end();
        if let Some(stripped) = trimmed.strip_suffix('\\') {
            let buf_start = buffer.len();
            if !stripped.is_empty() {
                buf_segments.push(SourceSegment {
                    prep_start: buf_start,
                    prep_end: buf_start + stripped.len(),
                    orig_start: line_start,
                    orig_end: line_start + stripped.len(),
                });
                buffer.push_str(stripped);
            }
            let space_start = buffer.len();
            let backslash_orig = line_start + stripped.len();
            buf_segments.push(SourceSegment {
                prep_start: space_start,
                prep_end: space_start + 1,
                orig_start: backslash_orig,
                orig_end: backslash_orig,
            });
            buffer.push(' ');
        } else {
            let buf_start = buffer.len();
            if !trimmed.is_empty() {
                buf_segments.push(SourceSegment {
                    prep_start: buf_start,
                    prep_end: buf_start + trimmed.len(),
                    orig_start: line_start,
                    orig_end: line_start + trimmed.len(),
                });
                buffer.push_str(trimmed);
            }
            flush_buffer(&mut result, &mut segments, &mut buffer, &mut buf_segments);
            let nl_start = result.len();
            let nl_orig = line_start + trimmed.len();
            segments.push(SourceSegment {
                prep_start: nl_start,
                prep_end: nl_start + 1,
                orig_start: nl_orig,
                orig_end: nl_orig,
            });
            result.push('\n');
        }
    }

    if !buffer.is_empty() {
        flush_buffer(&mut result, &mut segments, &mut buffer, &mut buf_segments);
        let nl_start = result.len();
        segments.push(SourceSegment {
            prep_start: nl_start,
            prep_end: nl_start + 1,
            orig_start: input.len(),
            orig_end: input.len(),
        });
        result.push('\n');
    }

    (
        result,
        SourceMap {
            segments,
            original_len: input.len(),
        },
    )
}
