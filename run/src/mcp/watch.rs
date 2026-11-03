//! Hot reload for MCP tools.
//!
//! `tools/list` already re-reads the Runfile on every request, but clients
//! cache the list and only ask again when told to. This module polls the
//! generated tool schema in a background thread and emits
//! `notifications/tools/list_changed` whenever it differs from the last
//! snapshot, so edits to the Runfile (or `~/.runfile`, or any `source`d file,
//! or a `set_cwd` to a different project) show up without restarting the session.
//!
//! Polling the *schema* (rather than file mtimes) means:
//! - sourced files are covered without tracking the include graph;
//! - edits that only change function bodies don't trigger a notification
//!   (bodies are re-read on every call anyway);
//! - no extra dependencies.

use super::tools::inspect;
use crate::config;
use std::time::Duration;

/// Environment variable controlling the poll interval in milliseconds.
/// `0` disables hot reload.
pub(super) const WATCH_INTERVAL_ENV_VAR: &str = "RUN_MCP_WATCH_INTERVAL_MS";
const DEFAULT_INTERVAL_MS: u64 = 1000;

/// Poll interval, or `None` when hot reload is disabled.
pub(super) fn watch_interval() -> Option<Duration> {
    let ms = std::env::var(WATCH_INTERVAL_ENV_VAR)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_INTERVAL_MS);
    (ms > 0).then(|| Duration::from_millis(ms))
}

/// Snapshot of the Runfile-derived tool schema.
///
/// `None` means the Runfile could not be parsed. Compared as JSON values so
/// `HashMap` iteration order doesn't cause spurious changes.
fn snapshot() -> Option<serde_json::Value> {
    let output = inspect().ok()?;
    serde_json::to_value(output.tools).ok()
}

/// Decide whether a new snapshot should trigger a notification.
///
/// Parse failures (`None`) never notify and never replace the last good
/// snapshot: a half-saved file shouldn't make tools flicker. Recovering from a
/// broken Runfile (last == None) does notify if the result parses.
fn should_notify(last: Option<&serde_json::Value>, current: Option<&serde_json::Value>) -> bool {
    match (last, current) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(a), Some(b)) => a != b,
    }
}

/// Spawn the background watcher. `notify` is called (from the watcher thread)
/// each time the tool list changes.
pub(super) fn spawn<F>(interval: Duration, notify: F)
where
    F: Fn() + Send + 'static,
{
    // Thread-local config doesn't cross threads; carry `--runfile` over explicitly.
    let custom_runfile = config::get_custom_runfile_path();

    let spawned = std::thread::Builder::new()
        .name("mcp-tools-watch".to_string())
        .spawn(move || {
            config::set_custom_runfile_path(custom_runfile);
            config::set_quiet_warnings(true);

            let mut last = snapshot();
            loop {
                std::thread::sleep(interval);
                let current = snapshot();
                if should_notify(last.as_ref(), current.as_ref()) {
                    last = current;
                    notify();
                }
            }
        });

    if let Err(e) = spawned {
        eprintln!("run: warning: could not start MCP tool watcher: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_notify() {
        let a = json!([{"name": "a"}]);
        let b = json!([{"name": "b"}]);

        assert!(!should_notify(Some(&a), Some(&a)));
        assert!(should_notify(Some(&a), Some(&b)));
        // Parse error: keep quiet
        assert!(!should_notify(Some(&a), None));
        assert!(!should_notify(None, None));
        // Recovered from a parse error
        assert!(should_notify(None, Some(&a)));
    }
}
