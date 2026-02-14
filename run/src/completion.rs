//! Shell completion installation and generation.

use clap::ValueEnum;
use std::fs;
use std::path::{Path, PathBuf};

// Embed completion scripts at compile time
const BASH_COMPLETION: &str = include_str!("../completions/run.bash");
const ZSH_COMPLETION: &str = include_str!("../completions/run.zsh");
const FISH_COMPLETION: &str = include_str!("../completions/run.fish");
const POWERSHELL_COMPLETION: &str = include_str!("../completions/run.ps1");

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell", alias = "pwsh")]
    PowerShell,
}

impl Shell {
    /// Returns the lowercase name of the shell.
    #[must_use] 
    pub fn name(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::PowerShell => "powershell",
        }
    }

    /// Returns the completion script content for this shell.
    #[must_use] 
    pub fn completion_script(self) -> &'static str {
        match self {
            Shell::Bash => BASH_COMPLETION,
            Shell::Zsh => ZSH_COMPLETION,
            Shell::Fish => FISH_COMPLETION,
            Shell::PowerShell => POWERSHELL_COMPLETION,
        }
    }

    /// Detect shell from the SHELL environment variable.
    #[must_use] 
    pub fn detect() -> Option<Shell> {
        let shell_var = std::env::var("SHELL").ok()?;
        if shell_var.contains("bash") {
            Some(Shell::Bash)
        } else if shell_var.contains("zsh") {
            Some(Shell::Zsh)
        } else if shell_var.contains("fish") {
            Some(Shell::Fish)
        } else if shell_var.contains("pwsh") || shell_var.contains("powershell") {
            Some(Shell::PowerShell)
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
pub fn install_completion_interactive(
    shell_opt: Option<Shell>,
    get_home_dir: impl Fn() -> Option<PathBuf>,
) {
    // Detect the shell if not provided
    let shell = shell_opt.or_else(Shell::detect).unwrap_or_else(|| {
        crate::fatal_error("Could not detect shell. Please specify: --install-completion <SHELL>\nSupported shells: bash, zsh, fish, powershell")
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
        Shell::PowerShell => install_powershell_completion(&home),
    }

    println!("\n✓ Installation complete!");
}

/// Write a completion file to the specified directory, creating the directory if needed.
fn write_completion_file(comp_dir: &PathBuf, filename: &str, content: &str) -> PathBuf {
    if let Err(e) = fs::create_dir_all(comp_dir) {
        crate::fatal_error(&format!("Error creating completion directory: {e}"));
    }

    let comp_file = comp_dir.join(filename);
    if let Err(e) = fs::write(&comp_file, content) {
        crate::fatal_error(&format!("Error writing completion file: {e}"));
    }

    comp_file
}

fn install_bash_completion(home: &Path) {
    // Install to ~/.local/share/bash-completion/completions/run
    let comp_dir = home.join(".local/share/bash-completion/completions");
    let comp_file = write_completion_file(&comp_dir, "run", BASH_COMPLETION);

    println!("✓ Installed completion to {}", comp_file.display());
    println!("\nTo activate completions, restart your shell or run:");
    println!("  source ~/.bashrc");
}

fn install_zsh_completion(home: &Path) {
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

fn install_fish_completion(home: &Path) {
    // Install to ~/.config/fish/completions/run.fish
    let comp_dir = home.join(".config/fish/completions");
    let comp_file = write_completion_file(&comp_dir, "run.fish", FISH_COMPLETION);

    println!("✓ Installed completion to {}", comp_file.display());
    println!("\nCompletions will be automatically loaded on next shell startup.");
    println!("To activate now, restart fish or run:");
    println!("  exec fish");
}

fn install_powershell_completion(home: &Path) {
    #[cfg(windows)]
    let comp_dir = home.join("Documents").join("PowerShell").join("Scripts");

    #[cfg(not(windows))]
    let comp_dir = home.join(".config").join("powershell");

    let comp_file = write_completion_file(&comp_dir, "run.ps1", POWERSHELL_COMPLETION);

    println!("✓ Installed completion script to {}", comp_file.display());

    // Determine PowerShell profile path
    #[cfg(windows)]
    let profile_dir = home.join("Documents").join("PowerShell");

    #[cfg(not(windows))]
    let profile_dir = home.join(".config").join("powershell");

    let profile_path = profile_dir.join("Microsoft.PowerShell_profile.ps1");

    // Create profile directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&profile_dir) {
        eprintln!("✗ Failed to create profile directory: {e}");
        return;
    }

    // Source line to add
    let source_line = format!(". \"{}\"", comp_file.display());

    // Check if already sourced
    let already_sourced = if profile_path.exists() {
        match std::fs::read_to_string(&profile_path) {
            Ok(content) => content
                .lines()
                .any(|line| line.trim() == source_line.trim() || line.contains("run.ps1")),
            Err(_) => false,
        }
    } else {
        false
    };

    if already_sourced {
        println!("✓ Completion already configured in PowerShell profile");
    } else {
        // Append to profile
        use std::fs::OpenOptions;
        use std::io::Write;

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&profile_path)
        {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "\n# run command completion\n{source_line}") {
                    eprintln!("✗ Failed to write to profile: {e}");
                } else {
                    println!("✓ Added completion to PowerShell profile");
                    println!("  Profile: {}", profile_path.display());
                }
            }
            Err(e) => {
                eprintln!("✗ Failed to open profile file: {e}");
            }
        }
    }

    println!("\nTo activate completions, restart PowerShell or run:");
    println!("  . $PROFILE");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_name_bash() {
        assert_eq!(Shell::Bash.name(), "bash");
    }

    #[test]
    fn test_shell_name_zsh() {
        assert_eq!(Shell::Zsh.name(), "zsh");
    }

    #[test]
    fn test_shell_name_fish() {
        assert_eq!(Shell::Fish.name(), "fish");
    }

    #[test]
    fn test_shell_name_powershell() {
        assert_eq!(Shell::PowerShell.name(), "powershell");
    }

    #[test]
    fn test_shell_completion_script_not_empty() {
        assert!(!Shell::Bash.completion_script().is_empty());
        assert!(!Shell::Zsh.completion_script().is_empty());
        assert!(!Shell::Fish.completion_script().is_empty());
        assert!(!Shell::PowerShell.completion_script().is_empty());
    }

    #[test]
    fn test_shell_completion_scripts_are_different() {
        let bash = Shell::Bash.completion_script();
        let zsh = Shell::Zsh.completion_script();
        let fish = Shell::Fish.completion_script();
        let pwsh = Shell::PowerShell.completion_script();
        assert_ne!(bash, zsh);
        assert_ne!(bash, fish);
        assert_ne!(bash, pwsh);
        assert_ne!(zsh, fish);
    }

    #[test]
    fn test_shell_detect_returns_option() {
        // Shell::detect() depends on SHELL env var; just verify it doesn't panic
        let _result = Shell::detect();
    }

    #[test]
    fn test_write_completion_file() {
        let temp = tempfile::tempdir().unwrap();
        let comp_dir = temp.path().join("completions");
        let comp_file = super::write_completion_file(
            &comp_dir,
            "test.sh",
            "# test completion",
        );
        assert!(comp_file.exists());
        assert_eq!(
            std::fs::read_to_string(&comp_file).unwrap(),
            "# test completion"
        );
    }
}
