# Recipes

Copy-pasteable Runfile snippets for common workflows. Adjust names and paths to fit your project.

## Docker workflows

```bash
# @desc Build the Docker image
docker:build() {
    docker build -t myapp:latest .
}

# @desc Start services
docker:up() {
    docker compose up -d
}

# @desc Tail logs for one or more services (defaults to app)
# @arg services Service names (optional)
docker:logs(...services) {
    if [ $# -eq 0 ]; then
        set -- app
    fi
    docker compose logs -f "$@"
}
```

Run with `run docker build`, `run docker up`, or `run docker logs api web`.

## CI pipeline

```bash
# @desc Lint the project
lint() cargo clippy -- -D warnings

# @desc Run tests
test() cargo test

# @desc Build release binary
build() cargo build --release

# @desc Full CI
ci() {
    echo "Running CI..."
    lint || exit 1
    test || exit 1
    build
}
```

## Polyglot data helpers

```bash
# @desc Analyze a JSON file
# @arg file Path to the JSON file
analyze(file: str) {
    #!/usr/bin/env python3
    import sys, json
    data = json.load(open(sys.argv[1]))
    print(f"Total records: {len(data)}")
}

# @desc Convert CSV to JSON
# @arg input Input CSV file
# @arg output Output JSON file
csv_to_json(input: str, output: str) {
    #!/usr/bin/env python3
    import sys, csv, json
    rows = list(csv.DictReader(open(sys.argv[1])))
    json.dump(rows, open(sys.argv[2], 'w'), indent=2)
    print(f"Converted {len(rows)} rows -> {sys.argv[2]}")
}
```

## Platform-specific commands

```bash
# @desc Clean build artifacts
# @os windows
clean() {
    del /Q /S target\*
}

# @desc Clean build artifacts
# @os unix
clean() {
    rm -rf target/
}

# @desc Open the project (portable)
open() {
    case "$(uname -s)" in
        Darwin) open . ;;
        Linux)  xdg-open . ;;
        MINGW*|MSYS*|CYGWIN*) start . ;;
        *) echo "Unsupported OS" >&2; return 1 ;;
    esac
}
```

## Deploy with dependencies

```bash
build() cargo build --release

# @desc Build and deploy
deploy(env: str, version = "latest") {
    build || exit 1
    echo "Deploying $version to $env"
    ./scripts/deploy.sh $env $version
}
```

For more patterns, combine these with the guidance in [Polyglot commands](./polyglot-commands.md) and [Command composition](./command-composition.md).
