//! # run
//!
//! A simple scripting language for CLI automation, inspired by shell scripting and Makefiles.

pub mod ast;
pub mod cli;
pub mod completion;
pub mod config;
pub mod executor;
pub mod interpreter;
pub mod mcp;
pub mod parser;
pub mod repl;
pub mod transpiler;
pub mod utils;

// Re-export the main CLI entry point for use by wrapper crates
// Note: This is defined in main.rs but we need to make it accessible
// The actual implementation will use a separate module or we include it here

/// Print an error message and exit with code 1.
pub fn fatal_error(message: &str) -> ! {
    eprintln!("{}", message);
    std::process::exit(1);
}
