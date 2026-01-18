# Shell Completions for `run`

The `run` command includes built-in shell completion scripts that provide tab-completion for:
- Command-line flags (`--list`, `--version`, etc.)
- Function names from your Runfile (dynamically loaded)
- Hierarchical completion for nested function names

## Hierarchical Completion

Functions defined with colons (e.g., `docker:shell`, `docker:logs`) are completed hierarchically:

```bash
run doc<TAB>      # Completes to: run docker
run docker <TAB>  # Shows: shell, logs
run docker sh<TAB> # Completes to: run docker shell
```

This means you type `run docker shell` instead of `run docker:shell`, making the command line more natural and intuitive.

## Quick Installation

The easiest way to install completions is with the `--install-completion` flag:

```bash
# Auto-detect your shell and install
run --install-completion

# Or specify a shell explicitly
run --install-completion zsh
run --install-completion bash
run --install-completion fish
run --install-completion powershell
```

This will:
- Create the completion file in the correct location for your shell
- Provide instructions for updating your shell config if needed
- Work for bash, zsh, fish, and powershell

After installation, restart your shell or follow the instructions shown.

## Manual Installation

If you prefer to install manually or need more control:

### Bash

Generate and install the completion script:

```bash
# Install to user completions directory
mkdir -p ~/.local/share/bash-completion/completions
run --generate-completion bash > ~/.local/share/bash-completion/completions/run

# Or install system-wide (requires sudo)
sudo run --generate-completion bash > /etc/bash_completion.d/run

# Or source directly in ~/.bashrc
echo 'eval "$(run --generate-completion bash)"' >> ~/.bashrc
```

Restart your shell or run `source ~/.bashrc` to activate.

### Zsh

Generate and install the completion script:

```zsh
# Install to user completions directory
mkdir -p ~/.zsh/completion
run --generate-completion zsh > ~/.zsh/completion/_run

# Add to ~/.zshrc (if not already present):
echo 'fpath=(~/.zsh/completion $fpath)' >> ~/.zshrc
echo 'autoload -Uz compinit && compinit' >> ~/.zshrc

# Or install to system-wide location
sudo run --generate-completion zsh > /usr/local/share/zsh/site-functions/_run
```

Restart your shell or run `exec zsh` to activate.

### Fish

Generate and install the completion script:

```fish
# Install to user completions directory
mkdir -p ~/.config/fish/completions
run --generate-completion fish > ~/.config/fish/completions/run.fish
```

Fish will automatically load completions from this directory on next shell startup.

### PowerShell

Generate and install the completion script:

```powershell
# Install to user config directory
New-Item -ItemType Directory -Force -Path ~/.config/powershell
run --generate-completion powershell > ~/.config/powershell/run.ps1

# Add to your PowerShell profile
Add-Content -Path $PROFILE -Value ". ~/.config/powershell/run.ps1"
```

Restart PowerShell or run `. $PROFILE` to activate.

## How It Works

The completion scripts are embedded in the `run` binary at compile time and dynamically read function names from your Runfile by calling `run --list`. This means:
- Completions automatically update when you change your Runfile
- Both local `./Runfile` and global `~/.runfile` functions are included
- No need to regenerate completions after adding new functions
- Easy installation - just one command per shell
- **Hierarchical completion** - colon-separated functions (like `docker:shell`) complete as space-separated commands (`docker shell`)
- **Intelligent caching** - completions are cached for 5 seconds per directory for speed

## Testing

After installation, restart your shell, then try:

```bash
run <TAB>          # Shows available top-level functions/namespaces and flags
run docker<TAB>    # If you have docker:* functions, completes to 'docker '
run docker <TAB>   # Shows subcommands like 'shell', 'logs'
run --<TAB>        # Shows available flags
```

## Available Shells

Both `--install-completion` and `--generate-completion` support:
- `bash`
- `zsh`
- `fish`
- `powershell` (or `pwsh`)

Example: `run --install-completion zsh`
