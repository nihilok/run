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

## Ad-hoc memory (SQLite recipe)

If you want memory-like behavior, you can do it with plain Runfile functions and an `sqlite3` database. The top-level `@instructions` lines in this example are appended to MCP `initialize.instructions`, so the agent gets usage guidance
at session start.

```bash
# @instructions Until built-in MCP memory mode is released, use this SQLite recipe for facts discovered during a session that need to survive context compaction or carry across sessions (for example: resolved environment details, confirmed decisions).
# @instructions Prefer the host's own auto-memory (for example: MEMORY.md) for user preferences and workflow instructions that should be read at conversation start.

# @desc Create ad-hoc memory tables (idempotent)
memory:init(db = ".run-memory.db") {
    sqlite3 "$db" "CREATE TABLE IF NOT EXISTS memories (id TEXT PRIMARY KEY, scope TEXT NOT NULL DEFAULT 'session', content TEXT NOT NULL, updated TEXT NOT NULL)"
    sqlite3 "$db" "CREATE TABLE IF NOT EXISTS tags (memory_id TEXT NOT NULL, tag TEXT NOT NULL, PRIMARY KEY (memory_id, tag))"
    sqlite3 "$db" "CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope)"
    sqlite3 "$db" "CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag)"
}

# @desc Store or update a memory note
# @arg content Note to remember
# @arg scope session|project|global (default: session)
# @arg tags Comma-separated tags (optional)
# @arg id Optional existing ID to upsert
memory:store(content: str, scope = "session", tags = "", id = "", db = ".run-memory.db") {
    memory:init "$db"
    entry_id="${id:-m-$(date +%s)-$RANDOM}"
    esc_content=$(printf "%s" "$content" | sed "s/'/''/g")
    esc_scope=$(printf "%s" "$scope" | sed "s/'/''/g")

    sqlite3 "$db" "INSERT INTO memories (id, scope, content, updated) VALUES ('$entry_id', '$esc_scope', '$esc_content', datetime('now'))
                   ON CONFLICT(id) DO UPDATE SET scope = excluded.scope, content = excluded.content, updated = datetime('now');"

    sqlite3 "$db" "DELETE FROM tags WHERE memory_id = '$entry_id';"
    IFS=',' read -ra parts <<< "$tags"
    for tag in "${parts[@]}"; do
        clean_tag=$(printf "%s" "$tag" | xargs)
        [ -z "$clean_tag" ] && continue
        esc_tag=$(printf "%s" "$clean_tag" | sed "s/'/''/g")
        sqlite3 "$db" "INSERT OR IGNORE INTO tags (memory_id, tag) VALUES ('$entry_id', '$esc_tag');"
    done

    echo "$entry_id"
}

# @desc Recall notes by substring/scope/id
memory:recall(query = "", scope = "", limit = 20, id = "", db = ".run-memory.db") {
    memory:init "$db"
    where="1=1"

    if [ -n "$query" ]; then
        esc_query=$(printf "%s" "$query" | sed "s/'/''/g")
        where="$where AND content LIKE '%$esc_query%'"
    fi
    if [ -n "$scope" ]; then
        esc_scope=$(printf "%s" "$scope" | sed "s/'/''/g")
        where="$where AND scope = '$esc_scope'"
    fi
    if [ -n "$id" ]; then
        esc_id=$(printf "%s" "$id" | sed "s/'/''/g")
        where="$where AND id = '$esc_id'"
    fi

    match_count=$(sqlite3 "$db" "SELECT COUNT(*) FROM memories WHERE $where;")
    if [ "$match_count" = "0" ]; then
        echo "No memories found for the provided filters." >&2
        return 1
    fi

    sqlite3 -header -column "$db" "SELECT id, scope, content, updated
                                   FROM memories
                                   WHERE $where
                                   ORDER BY updated DESC
                                   LIMIT $limit;"
    echo "Found $match_count matching memory note(s)." >&2
}

# @desc Forget a note by id
memory:forget(id: str, db = ".run-memory.db") {
    memory:init "$db"
    esc_id=$(printf "%s" "$id" | sed "s/'/''/g")
    tags_deleted=$(sqlite3 "$db" "DELETE FROM tags WHERE memory_id = '$esc_id'; SELECT changes();")
    memories_deleted=$(sqlite3 "$db" "DELETE FROM memories WHERE id = '$esc_id'; SELECT changes();")

    if [ "$memories_deleted" = "0" ]; then
        echo "No memory found for id '$esc_id'." >&2
        return 1
    fi

    echo "Deleted memory '$esc_id' (tags removed: $tags_deleted)." >&2
}

# @desc Run store/recall/forget in one verification flow
# @arg content Note to round-trip verify
# @arg scope session|project|global (default: session)
# @arg tags Comma-separated tags (optional)
# @arg id Optional existing ID to upsert
memory:roundtrip(content: str, scope = "session", tags = "", id = "", db = ".run-memory.db") {
    memory:init "$db"
    stored_id=$(memory:store "$content" "$scope" "$tags" "$id" "$db")
    esc_stored_id=$(printf "%s" "$stored_id" | sed "s/'/''/g")
    esc_scope=$(printf "%s" "$scope" | sed "s/'/''/g")
    esc_content=$(printf "%s" "$content" | sed "s/'/''/g")
    recalled_count=$(sqlite3 "$db" "SELECT COUNT(*) FROM memories WHERE id = '$esc_stored_id' AND scope = '$esc_scope' AND content = '$esc_content';")

    if [ "$recalled_count" = "0" ]; then
        echo "Round-trip recall verification failed for '$stored_id'." >&2
        return 1
    fi

    memory:forget "$stored_id" "$db"

    echo "$stored_id"
    echo "Round-trip verification succeeded for '$stored_id'." >&2
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
