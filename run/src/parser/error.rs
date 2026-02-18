//! User-friendly parse error types and formatting.
//!
//! Converts raw pest parser errors into structured, human-readable diagnostics
//! with source context, precise column indicators, and actionable hints.

use std::fmt;

use super::Rule;

/// A structured, user-friendly parser error.
///
/// Produced by converting a raw `pest::error::Error` and enriching it with
/// source context, translated rule names, and optional hints.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Human-readable error message (no raw rule names).
    pub message: String,
    /// Line number (1-indexed).
    pub line: usize,
    /// Column number (1-indexed) where the error begins.
    pub col: usize,
    /// End column for span errors (used to size the underline caret).
    pub col_end: Option<usize>,
    /// The full text of the offending source line.
    pub source_line: Option<String>,
    /// Optional source file name shown in the error header.
    pub filename: Option<String>,
    /// Optional suggestion to help the user fix the error.
    pub hint: Option<String>,
}

impl ParseError {
    /// Build a `ParseError` from a pest error, enriching it with source context.
    ///
    /// * `error`    – the raw pest error
    /// * `source`   – full source text that was being parsed
    /// * `filename` – optional file name to include in the error header
    pub fn from_pest(
        error: &pest::error::Error<Rule>,
        source: &str,
        filename: Option<&str>,
    ) -> Self {
        // Extract exact position from the pest error without string parsing.
        let (line, col, col_end) = match error.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l, c, None),
            pest::error::LineColLocation::Span((sl, sc), (el, ec)) => {
                let end = if sl == el { Some(ec) } else { None };
                (sl, sc, end)
            }
        };

        let source_line = source
            .lines()
            .nth(line.saturating_sub(1))
            .map(str::to_string);

        let (message, hint) = match &error.variant {
            pest::error::ErrorVariant::ParsingError {
                positives,
                negatives,
            } => {
                let msg = friendly_message(positives, negatives);
                let h = friendly_hint(positives, source_line.as_deref(), col);
                (msg, h)
            }
            pest::error::ErrorVariant::CustomError { message } => (message.clone(), None),
        };

        ParseError {
            message,
            line,
            col,
            col_end,
            source_line,
            filename: filename.map(str::to_string),
            hint,
        }
    }
}

/// Return a short, user-facing label for a grammar rule, or `None` to omit it.
///
/// Returning `None` suppresses the rule from user-visible messages (e.g. `EOI`
/// and internal atomic/silent rules are not useful to show).
fn rule_label(rule: Rule) -> Option<&'static str> {
    match rule {
        Rule::identifier => Some("identifier"),
        Rule::param_identifier => Some("parameter name"),
        Rule::rest_param => Some("`...name` (rest parameter)"),
        Rule::param_list => Some("parameter list"),
        Rule::param_type_annotation => Some("type annotation (`: type`)"),
        Rule::param_default => Some("default value (`= value`)"),
        Rule::block => Some("block body (`{ ... }`)"),
        Rule::command => Some("command"),
        Rule::function_def => Some("function definition"),
        Rule::function_call => Some("function call"),
        Rule::assignment => Some("variable assignment"),
        Rule::quoted_string => Some("quoted string"),
        Rule::variable => Some("variable (e.g. `$var`)"),
        Rule::value => Some("value"),
        Rule::word => Some("word"),
        Rule::operator => Some("operator"),
        Rule::argument => Some("argument"),
        Rule::argument_list => Some("argument list"),
        // EOI and all silent/atomic rules are suppressed.
        _ => None,
    }
}

/// Compose a human-readable message from the expected/unexpected rule sets.
fn friendly_message(positives: &[Rule], _negatives: &[Rule]) -> String {
    // Collect only rules we have friendly names for.
    let named: Vec<&str> = positives.iter().copied().filter_map(rule_label).collect();

    if named.is_empty() {
        return "unexpected token".to_string();
    }

    match named.as_slice() {
        [single] => format!("expected {single}"),
        [a, b] => format!("expected {a} or {b}"),
        many => {
            // Safety: `many` has ≥ 3 elements — split_last always succeeds here.
            if let Some((last, rest)) = many.split_last() {
                format!("expected {} or {}", rest.join(", "), last)
            } else {
                "unexpected token".to_string()
            }
        }
    }
}

/// Return an actionable hint based on the set of expected rules and context.
fn friendly_hint(positives: &[Rule], source_line: Option<&str>, col: usize) -> Option<String> {
    let has = |r: Rule| positives.contains(&r);

    // Missing function body.
    if has(Rule::block) && has(Rule::command) {
        return Some(
            "A function needs a body: put a command on the same line, \
             or wrap multiple commands in braces: `{ command1; command2 }`"
                .to_string(),
        );
    }

    // Broken parameter list.
    if has(Rule::param_identifier) || has(Rule::rest_param) {
        if let Some(line) = source_line {
            // Count unmatched opening parens up to the error column.
            let before_err = &line[..col.saturating_sub(1).min(line.len())];
            let open = before_err.chars().filter(|&c| c == '(').count();
            let close = before_err.chars().filter(|&c| c == ')').count();
            if open > close {
                return Some(
                    "A parameter list must be closed with `)`. \
                     Check for a missing `)` or a stray character inside the list."
                        .to_string(),
                );
            }
        }
        return Some(
            "Parameters look like `name`, `name: type`, or `...rest`. \
             Separate multiple parameters with commas."
                .to_string(),
        );
    }

    // Identifier expected but something else found.
    if has(Rule::identifier) && !has(Rule::command) {
        return Some(
            "Identifiers must start with a letter or `_` and contain only \
             letters, digits, `_`, or `:`."
                .to_string(),
        );
    }

    None
}

/// Format the caret underline for an error at `col` with optional `col_end`.
fn underline(col: usize, col_end: Option<usize>) -> String {
    let start = col.saturating_sub(1);
    let len = col_end.map_or(1, |end| end.saturating_sub(col).max(1));
    format!("{}{}", " ".repeat(start), "^".repeat(len))
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ── error header ────────────────────────────────────────────────────
        //   error: <message>
        //     --> <file>:<line>:<col>
        writeln!(f, "error: {}", self.message)?;

        let location = match &self.filename {
            Some(name) => format!("{name}:{}:{}", self.line, self.col),
            None => format!("{}:{}", self.line, self.col),
        };
        writeln!(f, "  --> {location}")?;

        // ── source context ──────────────────────────────────────────────────
        //    |
        // NN | <source line>
        //    | <caret>
        if let Some(ref src) = self.source_line {
            let num = self.line.to_string();
            let pad = " ".repeat(num.len());

            writeln!(f, "   {pad} |")?;
            writeln!(f, "   {num} | {src}")?;
            writeln!(f, "   {pad} | {}", underline(self.col, self.col_end))?;
        }

        // ── hint ─────────────────────────────────────────────────────────────
        if let Some(ref hint) = self.hint {
            writeln!(f)?;
            write!(f, "   = hint: {hint}")?;
        }

        Ok(())
    }
}

impl std::error::Error for ParseError {}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{super::ScriptParser, *};
    use pest::Parser;

    // The grammar's `command` rule is a permissive catch-all that accepts almost
    // any non-whitespace sequence, so "obviously wrong" strings like
    // `"function (bad"` actually parse successfully as commands.
    //
    // To reliably trigger a parse failure we use characters that cannot appear in
    // any `command_part`:
    //   - `"` that is never closed — starts a `quoted_string` that never closes,
    //     and `"` is excluded from `word`, so nothing can match it.
    //   - `,` at the start of a line — excluded from `word` and not an operator,
    //     so cannot start any item.

    /// Drive a real pest parse failure and convert it to `ParseError`.
    fn parse_err(input: &str, filename: Option<&str>) -> ParseError {
        let err = ScriptParser::parse(Rule::program, input)
            .expect_err("expected a parse failure for this input");
        ParseError::from_pest(&err, input, filename)
    }

    #[test]
    fn test_display_includes_filename_and_location() {
        let err = parse_err("\"unclosed string", Some("Runfile"));
        let rendered = err.to_string();
        assert!(
            rendered.contains("Runfile:"),
            "filename missing in:\n{rendered}"
        );
        assert!(
            rendered.contains("error:"),
            "'error:' prefix missing in:\n{rendered}"
        );
        assert!(
            rendered.contains("-->"),
            "location arrow missing in:\n{rendered}"
        );
    }

    #[test]
    fn test_display_without_filename() {
        let err = parse_err("\"unclosed string", None);
        let rendered = err.to_string();
        assert!(
            !rendered.contains("Runfile"),
            "unexpected filename in:\n{rendered}"
        );
        assert!(
            rendered.contains("-->"),
            "location arrow missing in:\n{rendered}"
        );
    }

    #[test]
    fn test_source_line_and_caret_present() {
        let input = "\"unclosed string here";
        let err = parse_err(input, Some("test.run"));
        let rendered = err.to_string();
        assert!(
            rendered.contains("unclosed string here"),
            "source line missing in:\n{rendered}"
        );
        assert!(rendered.contains('^'), "caret missing in:\n{rendered}");
    }

    #[test]
    fn test_no_raw_rule_names_in_message() {
        let inputs = ["\"unclosed", ",leading_comma"];
        for input in inputs {
            let err = parse_err(input, None);
            assert!(
                !err.message.contains("Rule::"),
                "raw rule name in message for `{input}`: {}",
                err.message
            );
        }
    }

    #[test]
    fn test_pos_location_extracted() {
        let err = parse_err("\"unclosed", Some("f"));
        assert!(err.line > 0, "line should be positive");
        assert!(err.col > 0, "col should be positive");
    }

    #[test]
    fn test_multiline_error_points_to_correct_line() {
        // Line 1 is a valid function definition; line 2 has an unclosed quote.
        let input = "ok() echo hello\n\"unclosed";
        let err = parse_err(input, None);
        assert_eq!(err.line, 2, "error should point to second line");
        assert!(
            err.source_line
                .as_deref()
                .unwrap_or("")
                .contains("unclosed"),
            "source_line should contain the bad token; got: {:?}",
            err.source_line
        );
    }

    #[test]
    fn test_hint_is_clean_when_present() {
        let err = parse_err("\"unclosed", None);
        // The hint is optional; when present it must not expose raw rule names.
        if let Some(ref hint) = err.hint {
            assert!(!hint.contains("Rule::"), "raw rule name in hint: {hint}");
        }
    }
}
