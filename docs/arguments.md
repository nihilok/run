# Arguments

`run` maps CLI arguments into shell variables based on the function signature. This page covers how parameters, defaults, and rest arguments work.

## Parameter mapping
- Signature parameters become shell variables with the same name.
  ```bash
  greet(name) echo "Hello, $name!"
  # run greet Alice  -> prints "Hello, Alice!"
  ```
- Parameters stay positional: the first CLI token maps to the first parameter, and so on.
- Legacy `$1`, `$2`, `$@` work alongside named parameters for backward compatibility.

## Defaults and required parameters
- Provide defaults in the signature to make parameters optional:
  ```bash
  deploy(env, version = "latest") echo "Deploying $version to $env"
  # run deploy staging        -> uses version "latest"
  # run deploy staging v1.2.3 -> overrides the default
  ```
- Calls missing a required argument fail before execution.
- Default values may be quoted; the target shell expands them.

## Rest parameters
Capture any remaining arguments into one variable:
```bash
echo_all(...args) echo "Args: $args"
# run echo_all foo bar -> Args: foo bar
```

## Types in signatures
Type hints (`str`, `int`, `bool`) are for documentation and MCP schema generation. Execution happens in the configured shell or interpreter, so convert inside your function if needed.

## Quoting and spaces
Arguments are passed as plain CLI tokens. Quote values containing spaces or shell-sensitive characters:
```bash
run deploy "staging us-east" "v1.2.3"
```

See [Variables](./variables.md) for environment handling and variable resolution.
