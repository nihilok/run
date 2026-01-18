//! # runtool
//!
//! This is a wrapper crate that re-exports the `run` binary.
//! The `runtool` crate provides the same functionality as `run` but under a different package name.
//!
//! This allows users to install the tool from either:
//! - `cargo install run`
//! - `cargo install runtool`
//!
//! Both commands install the same `run` binary.

fn main() {
    // Delegate to the run crate's CLI implementation
    run::cli::run_cli();
}
