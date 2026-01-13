//! Shell completion installation and generation.

use clap::ValueEnum;
use std::fs;
use std::path::PathBuf;

// Embed completion scripts at compile time
const BASH_COMPLETION: &str = include_str!("../completions/run.bash");
const ZSH_COMPLETION: &str = include_str!("../completions/run.zsh");
const FISH_COMPLETION: &str = include_str!("../completions/run.fish");

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    /// Returns the lowercase name of the shell.
    pub fn name(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
        }
    }

    /// Returns the completion script content for this shell.
    pub fn completion_script(self) -> &'static str {
        match self {
            Shell::Bash => BASH_COMPLETION,
            Shell::Zsh => ZSH_COMPLETION,
            Shell::Fish => FISH_COMPLETION,
        }
    }

    /// Detect shell from the SHELL environment variable.
    pub fn detect() -> Option<Shell> {
        let shell_var = std::env::var("SHELL").ok()?;
        if shell_var.contains("bash") {
            Some(Shell::Bash)
        } else if shell_var.contains("zsh") {
            Some(Shell::Zsh)
        } else if shell_var.contains("fish") {
            Some(Shell::Fish)
        } else {
            None
        }
    }
}

/// Generate shell completion script for the specified shell.
pub fn generate_completion_script(shell: Shell) {
    print!("{}", shell.completion_script());
}

/// Install shell completion interactively, detecting the shell and updating config files.
pub fn install_completion_interactive(shell_opt: Option<Shell>, get_home_dir: impl Fn() -> Option<PathBuf>) {
    // Detect the shell if not provided
    let shell = shell_opt.or_else(Shell::detect).unwrap_or_else(|| {
        crate::fatal_error("Could not detect shell. Please specify: --install-completion <SHELL>\nSupported shells: bash, zsh, fish")
    });

    println!(
        "Installing {} completion for {}...",
        shell.name(),
        env!("CARGO_PKG_NAME")
    );

    // Get home directory
    let home = get_home_dir()
        .unwrap_or_else(|| crate::fatal_error("Error: Could not determine home directory"));

    match shell {
        Shell::Bash => install_bash_completion(&home),
        Shell::Zsh => install_zsh_completion(&home),
        Shell::Fish => install_fish_completion(&home),
    }

    println!("\n✓ Installation complete!");
}

/// Write a completion file to the specified directory, creating the directory if needed.
fn write_completion_file(comp_dir: &PathBuf, filename: &str, content: &str) -> PathBuf {
    if let Err(e) = fs::create_dir_all(comp_dir) {
        crate::fatal_error(&format!("Error creating completion directory: {}", e));
    }

    let comp_file = comp_dir.join(filename);
    if let Err(e) = fs::write(&comp_file, content) {
        crate::fatal_error(&format!("Error writing completion file: {}", e));
    }

    comp_file
}

fn install_bash_completion(home: &PathBuf) {
    // Install to ~/.local/share/bash-completion/completions/run
    let comp_dir = home.join(".local/share/bash-completion/completions");
    let comp_file = write_completion_file(&comp_dir, "run", BASH_COMPLETION);

    println!("✓ Installed completion to {}", comp_file.display());
    println!("\nTo activate completions, restart your shell or run:");
    println!("  source ~/.bashrc");
}

fn install_zsh_completion(home: &PathBuf) {
    // Install to ~/.zsh/completion/_run
    let comp_dir = home.join(".zsh/completion");
    let comp_file = write_completion_file(&comp_dir, "_run", ZSH_COMPLETION);

    println!("✓ Installed completion to {}", comp_file.display());

    // Check if .zshrc needs updating
    let zshrc = home.join(".zshrc");
    let needs_fpath = if zshrc.exists() {
        let content = fs::read_to_string(&zshrc).unwrap_or_default();
        // Check each non-comment, non-empty line for fpath including ~/.zsh/completion
        !content.lines().any(|line| {
            let line = line.trim_start();
            // Ignore comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                return false;
            }
            // Look for fpath assignment including ~/.zsh/completion
            line.contains("fpath") && line.contains("~/.zsh/completion")
        })
    } else {
        true
    };

    let needs_compinit = if zshrc.exists() {
        let content = fs::read_to_string(&zshrc).unwrap_or_default();
        !content.contains("autoload -Uz compinit")
    } else {
        true
    };

    if needs_fpath || needs_compinit {
        println!("\nAdd the following to your ~/.zshrc:");
        if needs_fpath {
            println!("  fpath=(~/.zsh/completion $fpath)");
        }
        if needs_compinit {
            println!("  autoload -Uz compinit && compinit");
        }
        println!("\nOr run:");
        if needs_fpath {
            println!("  echo 'fpath=(~/.zsh/completion $fpath)' >> ~/.zshrc");
        }
        if needs_compinit {
            println!("  echo 'autoload -Uz compinit && compinit' >> ~/.zshrc");
        }
    }

    println!("\nTo activate completions, restart your shell or run:");
    println!("  exec zsh");
}

fn install_fish_completion(home: &PathBuf) {
    // Install to ~/.config/fish/completions/run.fish
    let comp_dir = home.join(".config/fish/completions");
    let comp_file = write_completion_file(&comp_dir, "run.fish", FISH_COMPLETION);

    println!("✓ Installed completion to {}", comp_file.display());
    println!("\nCompletions will be automatically loaded on next shell startup.");
    println!("To activate now, restart fish or run:");
    println!("  exec fish");
}

