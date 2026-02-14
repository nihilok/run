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

## Polyglot parameter mapping
Named parameters also work in Python, Node.js, and Ruby functions. `run` auto-generates variable declarations so you can use parameter names directly:

```bash
# @shell python
greet(name, greeting = "Hello") {
    print(f"{greeting}, {name}!")
}
```

Type hints (`int`, `bool`) apply real conversions in polyglot scripts: `int` wraps in `int()`/`parseInt()`/`.to_i`, and `bool` performs truth-checking. See [Polyglot commands](./polyglot-commands.md) for the full mapping table.

## Types in signatures
Type hints (`str`, `int`, `bool`) are used for MCP schema generation. In shell functions, conversion is up to your script. In polyglot functions (Python, Node.js, Ruby), `int` and `bool` types are automatically converted when parameters are injected.

## Quoting and spaces
Arguments are passed as plain CLI tokens. Quote values containing spaces or shell-sensitive characters:
```bash
run deploy "staging us-east" "v1.2.3"
```

See [Variables](./variables.md) for environment handling and variable resolution.
