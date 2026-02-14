# Polyglot commands

Mix shell, Python, Node, and other interpreters in a single `Runfile`.

## How polyglot execution works
- **Shebangs** (`#!/usr/bin/env python3`, `#!/usr/bin/env node`) tell `run` which interpreter to use for the body.
- **`@shell`** overrides or replaces a shebang when you want explicit control.
- **Named parameters** declared in the function signature are auto-injected as variables in the script body. No manual unpacking of `sys.argv` or `process.argv` needed.
- Arguments are also forwarded positionally (`sys.argv`, `process.argv`, `ARGV`), so you can still use manual access when you prefer.

## Named parameters in polyglot functions

When a polyglot function declares parameters in its signature, `run` generates a preamble that creates proper named variables in the target language. This matches how shell functions already get `$name` substitution.

```bash
# @shell python
greet(name, greeting = "Hello") {
    print(f"{greeting}, {name}!")
}
```

```
run greet World       -> Hello, World!
run greet World Hi    -> Hi, World!
```

The same works for Node.js and Ruby:

```bash
# @shell node
greet(name, greeting = "Hello") {
    console.log(`${greeting}, ${name}!`);
}

# @shell ruby
greet(name, greeting = "Hello") {
    puts "#{greeting}, #{name}!"
}
```

### What gets generated

For each named parameter, `run` prepends variable declarations before your script body:

| Feature | Python | Node.js | Ruby |
|---|---|---|---|
| Required param | `name = sys.argv[1]` | `const name = process.argv[1];` | `name = ARGV[0]` |
| Default value | `name = sys.argv[1] if len(sys.argv) > 1 else "default"` | `const name = process.argv.length > 1 ? process.argv[1] : "default";` | `name = ARGV.length > 0 ? ARGV[0] : "default"` |
| Rest param | `args = sys.argv[2:]` | `const args = process.argv.slice(2);` | `args = ARGV[1..]` |
| `int` type | `int(sys.argv[1])` | `parseInt(process.argv[1], 10)` | `ARGV[0].to_i` |
| `float` type | `float(sys.argv[1])` | `parseFloat(process.argv[1])` | `ARGV[0].to_f` |
| `bool` type | `sys.argv[1].lower() in ('true', '1', 'yes')` | `!['false','0',''].includes(...)` | `!['false','0',''].include?(...)` |
| `object` type | `json.loads(sys.argv[1])` | `JSON.parse(process.argv[1])` | `JSON.parse(ARGV[0])` |

When `object` is used, `import json` (Python) or `require 'json'` (Ruby) is added automatically. Node.js needs no extra import since `JSON` is a global.

### Manual access still works

The `$name` text substitution and positional `sys.argv`/`process.argv`/`ARGV` access continue to work. Named parameters are additive and don't conflict with either approach.

```bash
# @desc Analyze a JSON file
analyze(file: str) {
    #!/usr/bin/env python3
    import sys, json
    data = json.load(open(file))       # uses the auto-injected variable
    # data = json.load(open(sys.argv[1]))  # manual access also works
    print(f"Records: {len(data)}")
}

# @desc Start a dev server
# @shell node
dev:server(port = "3000") {
    const p = parseInt(port, 10);      # uses the auto-injected variable
    require('http').createServer((_, res) => res.end('ok')).listen(p);
    console.log(`Listening on ${p}`);
}
```

## Best practices
- Prefer named parameters over manual `sys.argv`/`process.argv` unpacking for clarity.
- Keep interpreter-specific logic inside dedicated functions; orchestrate with shell functions for portability.
- Use `@desc` and `@arg` for clearer MCP schemas.

See [Attributes and interpreters](./attributes-and-interpreters.md) for selection rules, and [Command composition](./command-composition.md) for combining polyglot functions with others.
