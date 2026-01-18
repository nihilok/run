# RFC 006: Structured Output for MCP and AI Agents

**Status**: Draft | **Type**: Enhancement | **Target**: v0.4.0  
**Topic**: MCP Output Formatting, Agent Usability

## 1. Summary

This proposal enhances the output capture and formatting of `run` command execution to provide structured, context-rich responses for AI agents consuming MCP tool results.

Currently, command outputs are streamed directly to stdout/stderr without structured capture. This RFC introduces an execution capture layer that collects output, exit codes, timing, and execution context—then formats them in a way that maximizes utility for LLM consumption.

---

## 2. Motivation

When an AI agent calls a Runfile function via MCP, the current output is raw text. This creates several problems:

### 2.1. Lack of Execution Context

Consider these Runfile functions:

```bash
webserver() ssh -i ~/.ssh/key.pem admin@18.133.231.119
mailserver() ssh -T -o LogLevel=QUIET ubuntu@51.89.164.131

# @desc Connect to the webserver and run a command
# @arg command The command to run on the webserver
on_webserver(command: str) {
  echo "$command" | webserver
}

# @desc Update both servers
update_servers() {
  on_webserver "sudo apt update && sudo apt upgrade -y"
  on_mailserver "sudo apt update && sudo apt upgrade -y"
}
```

When an agent calls `update_servers`, it receives a blob of interleaved apt output with no indication of:
- Which server produced which output
- Whether each command succeeded or failed
- How long each operation took
- The logical structure of the multi-step operation

### 2.2. No Error Differentiation

stdout and stderr are often mixed or lost. The agent cannot distinguish between:
- Informational output
- Warnings
- Fatal errors

### 2.3. Missing Metadata

No timing information, exit codes, or command context is provided—making it difficult for the agent to reason about partial failures or performance issues.

---

## 3. Design

### 3.1. Core Data Structures

#### CommandOutput

Captures the result of a single command execution:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    /// The command that was executed
    pub command: String,
    
    /// Captured standard output
    pub stdout: String,
    
    /// Captured standard error
    pub stderr: String,
    
    /// Process exit code (None if killed by signal)
    pub exit_code: Option<i32>,
    
    /// Execution duration in milliseconds
    pub duration_ms: u128,
    
    /// Timestamp when execution started (Unix epoch ms)
    pub started_at: u128,
}
```

#### ExecutionContext

Provides contextual information about where/how commands ran:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// The Runfile function name that was invoked
    pub function_name: String,
    
    /// Remote host if SSH detected (extracted from command)
    pub remote_host: Option<String>,
    
    /// Remote user if SSH detected
    pub remote_user: Option<String>,
    
    /// Shell/interpreter used (@shell attribute value)
    pub interpreter: String,
    
    /// Working directory
    pub working_directory: Option<String>,
}
```

#### StructuredResult

The complete result returned to MCP clients:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredResult {
    /// Execution context
    pub context: ExecutionContext,
    
    /// Individual command outputs (in execution order)
    pub outputs: Vec<CommandOutput>,
    
    /// Overall success (all commands exited 0)
    pub success: bool,
    
    /// Total execution time
    pub total_duration_ms: u128,
    
    /// Human-readable summary
    pub summary: String,
}
```

### 3.2. Output Modes

The interpreter supports multiple output modes:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputMode {
    /// Stream directly to terminal (current behavior, default for CLI)
    #[default]
    Stream,
    
    /// Capture all output for programmatic access
    Capture,
    
    /// Capture and format as structured JSON (for MCP)
    Structured,
}
```

### 3.3. SSH Context Extraction

For remote execution functions, extract connection details:

```rust
impl ExecutionContext {
    /// Parse SSH commands to extract remote execution context
    pub fn extract_ssh_context(command: &str) -> Option<(String, String)> {
        // Match patterns like:
        //   ssh user@host
        //   ssh -i key.pem user@host
        //   ssh -T -o LogLevel=QUIET user@host
        let re = Regex::new(r"ssh\s+(?:-\S+\s+)*(\w+)@([\w.-]+)").ok()?;
        let caps = re.captures(command)?;
        
        Some((
            caps.get(1)?.as_str().to_string(),  // user
            caps.get(2)?.as_str().to_string(),  // host
        ))
    }
}
```

---

## 4. Implementation

### 4.1. Modify Shell Execution

Update `run/src/interpreter/shell.rs` to support capture mode:

```rust
impl ShellExecutor {
    pub fn execute_with_capture(
        &self,
        command: &str,
        shell: &str,
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
        let start = Instant::now();
        
        let output = Command::new(shell)
            .arg("-c")
            .arg(command)
            .output()?;
        
        Ok(CommandOutput {
            command: command.to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            duration_ms: start.elapsed().as_millis(),
            started_at,
        })
    }
}
```

### 4.2. Update Interpreter State

Add output tracking to the interpreter:

```rust
pub struct Interpreter {
    // ...existing fields...
    
    /// Output capture mode
    output_mode: OutputMode,
    
    /// Captured outputs when in Capture/Structured mode
    captured_outputs: Vec<CommandOutput>,
    
    /// Current execution context
    current_context: Option<ExecutionContext>,
}

impl Interpreter {
    pub fn set_output_mode(&mut self, mode: OutputMode) {
        self.output_mode = mode;
    }
    
    pub fn take_captured_outputs(&mut self) -> Vec<CommandOutput> {
        std::mem::take(&mut self.captured_outputs)
    }
}
```

### 4.3. MCP Handler Integration

Update `run/src/mcp/handlers.rs` to use structured output:

```rust
pub fn handle_call_tool(
    &mut self,
    name: &str,
    arguments: &serde_json::Value,
) -> Result<CallToolResult, McpError> {
    // Set structured mode for MCP execution
    self.interpreter.set_output_mode(OutputMode::Structured);
    
    // Execute the function
    let result = self.interpreter.execute_function(name, args)?;
    
    // Collect structured result
    let outputs = self.interpreter.take_captured_outputs();
    let structured = StructuredResult::from_outputs(name, outputs);
    
    // Format for MCP response
    Ok(CallToolResult {
        content: vec![Content::Text {
            text: structured.to_mcp_format(),
        }],
        is_error: Some(!structured.success),
    })
}
```

### 4.4. Output Formatting

Provide multiple format options for different consumers:

```rust
impl StructuredResult {
    /// Format as JSON for programmatic consumption
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
    
    /// Format as Markdown for LLM readability
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        
        // Header with context
        md.push_str(&format!("## Execution: `{}`\n\n", self.context.function_name));
        
        if let Some(host) = &self.context.remote_host {
            md.push_str(&format!("**Host:** {}@{}\n", 
                self.context.remote_user.as_deref().unwrap_or("?"), 
                host
            ));
        }
        
        md.push_str(&format!("**Status:** {}\n", 
            if self.success { "✓ Success" } else { "✗ Failed" }
        ));
        md.push_str(&format!("**Duration:** {}ms\n\n", self.total_duration_ms));
        
        // Individual command outputs
        for (i, output) in self.outputs.iter().enumerate() {
            md.push_str(&format!("### Step {} ({}ms)\n", i + 1, output.duration_ms));
            md.push_str(&format!("`{}`\n\n", output.command));
            
            if !output.stdout.is_empty() {
                md.push_str("**Output:**\n```\n");
                md.push_str(&output.stdout);
                md.push_str("```\n\n");
            }
            
            if !output.stderr.is_empty() {
                md.push_str("**Errors:**\n```\n");
                md.push_str(&output.stderr);
                md.push_str("```\n\n");
            }
            
            if let Some(code) = output.exit_code {
                if code != 0 {
                    md.push_str(&format!("**Exit Code:** {}\n\n", code));
                }
            }
        }
        
        md
    }
    
    /// Format optimized for MCP tool response (clean markdown, no JSON)
    pub fn to_mcp_format(&self) -> String {
        self.to_markdown()
    }
}
```

---

## 5. CLI Integration

### 5.1. New Flag: `--output-format`

```bash
# Default: stream to terminal
run deploy

# Capture and output as JSON
run --output-format=json deploy

# Capture and output as structured markdown
run --output-format=markdown deploy
```

### 5.2. MCP Server Behavior

When running as MCP server (`--serve-mcp`), structured output is automatic:

- All tool calls use `OutputMode::Structured`
- Results include context, timing, and exit codes
- Format is optimized for LLM consumption

---

## 6. Example Output

### Input Runfile

```bash
# @desc Update both servers
update_servers() {
  on_webserver "sudo apt update && sudo apt upgrade -y"
  on_mailserver "sudo apt update && sudo apt upgrade -y"
}
```

### MCP Tool Response

```markdown
## Execution: `update_servers`

**Status:** ✓ Success
**Duration:** 45230ms

### Step 1 (22150ms)
`echo "sudo apt update && sudo apt upgrade -y" | ssh admin@18.133.231.119`

**Output:**
```
Hit:1 http://deb.debian.org/debian bookworm InRelease
Reading package lists... Done
Building dependency tree... Done
0 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.
```

### Step 2 (23080ms)
`echo "sudo apt update && sudo apt upgrade -y" | ssh ubuntu@51.89.164.131`

**Output:**
```
Hit:1 http://archive.ubuntu.com/ubuntu jammy InRelease
Reading package lists... Done
Building dependency tree... Done
0 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.
```
```

---

## 7. Implementation Phases

### Phase 1: Core Capture Infrastructure
- [ ] Add `CommandOutput` and `StructuredResult` types to `ast.rs`
- [ ] Add `OutputMode` enum to interpreter
- [ ] Implement `execute_with_capture` in shell executor
- [ ] Add output collection to interpreter state

### Phase 2: MCP Integration
- [ ] Update MCP handlers to use structured mode
- [ ] Implement `to_mcp_format()` formatting
- [ ] Add SSH context extraction
- [ ] Test with multi-step functions

### Phase 3: CLI Enhancement
- [ ] Add `--output-format` flag
- [ ] Update help text and documentation
- [ ] Add integration tests for output formats

### Phase 4: Advanced Features
- [ ] Streaming capture (for long-running commands)
- [ ] Output truncation for very large results
- [ ] Configurable verbosity levels

---

## 8. Testing Strategy

### Unit Tests
- `CommandOutput` serialization/deserialization
- SSH context extraction regex
- Markdown formatting output
- Exit code propagation

### Integration Tests
- Multi-command function capture
- Error handling and partial failures
- MCP tool response format validation
- Large output handling

### Manual Testing
- Real SSH command execution
- Claude Desktop MCP integration
- Various shell interpreters (bash, zsh, fish)

---

## 9. Security Considerations

- **Output Sanitization**: Avoid leaking sensitive data in structured output
- **Size Limits**: Cap captured output to prevent memory exhaustion
- **Credential Detection**: Consider warning if output contains patterns like API keys

---

## 10. Future Considerations

- **Streaming Results**: For long-running commands, consider SSE-style updates
- **Output Schemas**: Allow functions to declare expected output format with `@output json`
- **Result Caching**: Cache results of idempotent functions
- **Diff Mode**: Show changes between consecutive runs
