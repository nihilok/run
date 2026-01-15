//! # run
//!
//! A simple scripting language for CLI automation, inspired by shell scripting and Makefiles.

pub mod ast;
pub mod completion;
pub mod config;
pub mod executor;
pub mod interpreter;
pub mod mcp;
pub mod parser;
pub mod repl;
pub mod utils;

/// Print an error message and exit with code 1.
pub fn fatal_error(message: &str) -> ! {
    eprintln!("{}", message);
    std::process::exit(1);
}
