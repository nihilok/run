use inquire::MultiSelect;
use std::fs;
use std::path::PathBuf;

pub fn interactive_setup() {
    println!("Welcome to the AI Agent integration setup!");

    let options = vec![
        "Antigravity (Global MCP & ./.agents/)",
        "Claude Code (./.claude.json)",
        "Codex (~/.codex/config.toml)",
        "Cursor (./.cursor/rules/)",
        "GitHub Copilot (Global MCP & ./.github/)",
    ];

    let selection = MultiSelect::new(
        "Which AI assistants would you like to configure for this project?",
        options,
    )
    .with_help_message("Press Space to select, Enter to confirm")
    .prompt();

    match selection {
        Ok(agents) => {
            if agents.is_empty() {
                println!("No agents selected. Setup canceled.");
                return;
            }

            for agent in agents {
                println!("\nConfiguring {agent}...");
                if agent.starts_with("Antigravity") {
                    setup_antigravity();
                } else if agent.starts_with("Claude") {
                    setup_claude();
                } else if agent.starts_with("Codex") {
                    setup_codex();
                } else if agent.starts_with("Cursor") {
                    setup_cursor();
                } else if agent.starts_with("GitHub") {
                    setup_copilot();
                }
            }
            println!("\n✅ Setup complete! Your AI agents are now ready to use 'run' natively.");
        }
        Err(_) => println!("Installation canceled."),
    }
}

fn setup_antigravity() {
    // 1. Register MCP globally
    if let Some(mut home) = crate::config::get_home_dir() {
        home.push(".gemini");
        home.push("config");
        let _ = fs::create_dir_all(&home);

        home.push("mcp_config.json");
        let mcp_content = r#"{
  "mcpServers": {
    "run": {
      "command": "run",
      "args": ["--serve-mcp"]
    }
  }
}"#;
        if home.exists() {
            println!(
                "✓ Antigravity MCP config already exists at {}. Please ensure `run --serve-mcp` is registered.",
                home.display()
            );
        } else {
            let _ = fs::write(&home, mcp_content);
            println!("✓ Registered MCP server globally in {}", home.display());
        }
    }

    // 2. Write local breadcrumb
    let _ = fs::create_dir_all(".agents/rules");
    let content = r"# Runfile Authoring

When you are asked to create, modify, or debug a `Runfile`, you MUST use the `run_docs` MCP tool to read the `runfile-syntax` and `attributes-and-interpreters` topics before you start writing code.

**CRITICAL:** The `run` MCP server exposes tasks from the `Runfile` as tools. If you add, rename, or remove a task in the `Runfile`, you MUST explicitly ask the user to restart the AI session (or reload the window) so the MCP client can pick up the new tools.
";
    if let Err(e) = fs::write(".agents/rules/AGENTS.md", content) {
        println!("❌ Failed to write Antigravity rules: {e}");
    } else {
        println!("✓ Created local breadcrumb in .agents/rules/AGENTS.md");
    }
}

fn setup_cursor() {
    let _ = fs::create_dir_all(".cursor/rules");
    let content = r#"---
description: Runfile Authoring
globs: ["Runfile"]
---
# Runfile Authoring

When you are asked to create, modify, or debug a `Runfile`, you MUST use the `run_docs` MCP tool to read the `runfile-syntax` and `attributes-and-interpreters` topics before you start writing code.

**CRITICAL:** The `run` MCP server exposes tasks from the `Runfile` as tools. If you add, rename, or remove a task in the `Runfile`, you MUST explicitly ask the user to restart the AI session (or reload the window) so the MCP client can pick up the new tools.
"#;
    if let Err(e) = fs::write(".cursor/rules/runfile.mdc", content) {
        println!("❌ Failed to write Cursor rules: {e}");
    } else {
        println!("✓ Created .cursor/rules/runfile.mdc");
    }
}

fn setup_copilot() {
    // 1. Register MCP globally
    if let Some(mut home) = crate::config::get_home_dir() {
        home.push(".copilot");
        let _ = fs::create_dir_all(&home);

        home.push("mcp-config.json");
        let content = r#"{
  "mcpServers": {
    "run": {
      "command": "run",
      "args": ["--serve-mcp"]
    }
  }
}"#;
        if home.exists() {
            println!(
                "✓ Copilot MCP config found at {}. Please ensure `run --serve-mcp` is registered.",
                home.display()
            );
        } else if let Err(e) = fs::write(&home, content) {
            println!("❌ Failed to write {}: {e}", home.display());
        } else {
            println!(
                "✓ Created {} to register MCP server globally",
                home.display()
            );
        }
    } else {
        println!("❌ Could not determine home directory for Copilot configuration.");
    }

    // 2. Write local breadcrumb
    let _ = fs::create_dir_all(".github");
    let content = "\n\nWhen writing a Runfile, always use the `run_docs` MCP tool to look up the correct syntax first. If you modify the Runfile to add, rename, or remove tasks, you MUST explicitly ask the user to restart their AI session (or reload the window) so the MCP client can pick up the new tools.\n";
    if let Ok(existing) = fs::read_to_string(".github/copilot-instructions.md") {
        if existing.contains("Runfile") {
            println!("✓ .github/copilot-instructions.md already contains Runfile instructions");
        } else {
            let _ = fs::write(".github/copilot-instructions.md", existing + content);
            println!("✓ Appended local breadcrumb to .github/copilot-instructions.md");
        }
    } else {
        let _ = fs::write(".github/copilot-instructions.md", content.trim());
        println!("✓ Created local breadcrumb in .github/copilot-instructions.md");
    }
}

fn setup_claude() {
    let _ = fs::create_dir_all(".claude");
    let content = r#"{
  "mcpServers": {
    "run": {
      "command": "run",
      "args": ["--serve-mcp"]
    }
  }
}"#;
    if PathBuf::from(".claude.json").exists()
        || PathBuf::from(".claude/settings.local.json").exists()
    {
        println!(
            "✓ Claude config found. Please ensure `run --serve-mcp` is added to your mcpServers block."
        );
    } else if let Err(e) = fs::write(".claude.json", content) {
        println!("❌ Failed to write .claude.json: {e}");
    } else {
        println!("✓ Created .claude.json to register MCP server");
    }
}

fn setup_codex() {
    if let Some(mut home) = crate::config::get_home_dir() {
        home.push(".codex");
        let _ = fs::create_dir_all(&home);

        home.push("config.toml");
        let toml_content = "\n[mcpServers.run]\ncommand = \"run\"\nargs = [\"--serve-mcp\"]\n";

        if let Ok(existing) = fs::read_to_string(&home) {
            if existing.contains("[mcpServers.run]") {
                println!(
                    "✓ Codex MCP config already contains 'run' at {}",
                    home.display()
                );
            } else {
                let _ = fs::write(&home, existing + toml_content);
                println!("✓ Appended MCP server to {}", home.display());
            }
        } else {
            let _ = fs::write(&home, toml_content.trim_start());
            println!(
                "✓ Created {} to register MCP server globally",
                home.display()
            );
        }
    } else {
        println!("❌ Could not determine home directory for Codex configuration.");
    }
}
