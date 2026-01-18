//! # run
//!
//! A simple scripting language for CLI automation, inspired by shell scripting and Makefiles.
//! Define functions in a `Runfile` (or `~/.runfile`) and call them from the command line to streamline your development workflow.
//!
//! ## Usage
//!
//! - Run a script file: `run myscript.run`
//! - Call a function: `run build`, `run docker shell app`
//! - Pass arguments: `run start dev`, `run git commit "Initial commit"`
//! - Interactive shell: `run`
//!
//! See README.md for more details and examples.

/// Entry point for the CLI tool.
fn main() {
    run::cli::run_cli();
}

