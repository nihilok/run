# Polyglot commands

Mix shell, Python, Node, and other interpreters in a single `Runfile`.

## How polyglot execution works
- **Shebangs** (`#!/usr/bin/env python3`, `#!/usr/bin/env node`) tell `run` which interpreter to use for the body.
- **`@shell`** overrides or replaces a shebang when you want explicit control.
- Arguments are forwarded positionally (`sys.argv[1]`, `process.argv[2]`, etc.), after defaults from the signature are applied.

```bash
# @desc Analyze a JSON file
analyze(file: str) {
    #!/usr/bin/env python3
    import sys, json
    data = json.load(open(sys.argv[1]))
    print(f"Records: {len(data)}")
}

# @desc Start a dev server
# @shell node
dev:server(port = "3000") {
    const port = process.argv[2] || 3000;
    require('http').createServer((_, res) => res.end('ok')).listen(port);
    console.log(`Listening on ${port}`);
}
```

## Best practices
- Keep interpreter-specific logic inside dedicated functions; orchestrate with shell functions for portability.
- Use `@desc` and `@arg` for clearer MCP schemas.
- Prefer `@shell` when you need to override a shebang or keep the first line free for comments.

See [Attributes and interpreters](./attributes-and-interpreters.md) for selection rules, and [Command composition](./command-composition.md) for combining polyglot functions with others.
