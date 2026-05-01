# Development Quick Reference

Essential commands and workflows for working on Atomic.

## Startup

```bash
# Initial setup
npm install
cargo check

# Desktop dev (full stack with hot reload)
npm run tauri dev

# Server + Web (two terminals)
npm run dev:server          # Terminal 1
npm run dev:web             # Terminal 2 → http://localhost:1420

# Frontend only (existing server at :44380)
npm run dev

# Standalone server
cargo run -p atomic-server -- serve --port 8080
```

## Build

```bash
# Desktop production
npm run tauri build

# Server binary + web static
npm run build:server
npm run build:web

# Mobile
npm run build:mobile
npm run build:mobile:ios
npm run build:mobile:android

# MCP bridge
npm run build:mcp-bridge
```

## Testing

```bash
# Run all tests
cargo test
npm test

# Watch mode (frontend)
npm test -- --watch

# Specific test
cargo test -p atomic-core embedding_deduplication
npm test -- atoms.test.ts

# Coverage
npm test -- --coverage

# Integration tests
npm run stress:wikipedia
```

## Checks

```bash
# TypeScript
npx tsc --noEmit

# Rust
cargo check
cargo fmt
cargo clippy -- -D warnings

# All checks before commit
cargo fmt && cargo clippy && cargo test && npm test && npx tsc --noEmit
```

## Database

```bash
# Reset (SQLite)
npm run db:reset

# Reset specific tables
npm run db:reset-tags
npm run db:reset-chunks

# Query
sqlite3 databases/default.db "SELECT COUNT(*) FROM atoms;"
sqlite3 databases/registry.db "SELECT key, value FROM settings;"

# List databases
sqlite3 databases/registry.db "SELECT id, name, is_default FROM databases;"

# Inspect embedding status
sqlite3 databases/default.db "SELECT embedding_status, COUNT(*) FROM atoms GROUP BY embedding_status;"
```

## Debugging

```bash
# Rust logging
RUST_LOG=debug cargo run -p atomic-server -- serve --port 8080
RUST_LOG=atomic_core::embedding=debug npm run tauri dev

# Backtrace
RUST_BACKTRACE=1 cargo run -p atomic-server -- serve --port 8080

# Browser DevTools in Tauri
Right-click → Inspect Element (or Cmd+Option+I)
```

## Git Workflow

```bash
# Create feature branch
git checkout -b feature/your-feature

# Commit
git add .
git commit -m "feat(scope): clear description"

# Push
git push origin feature/your-feature

# Fetch updates from upstream
git fetch upstream
git rebase upstream/main
```

## Common Issues

| Issue | Solution |
|-------|----------|
| sqlite-vec not found | `brew install sqlite pkg-config` + `cargo clean && cargo build` |
| npm modules not found | `npm install && npm run postinstall` |
| Port 8080 in use | `lsof -i :8080` or use `--port 9000` |
| Tauri won't start | `rm -rf src-tauri/target && npm run tauri dev` |
| Embedding stuck | Check Settings → AI Provider, verify API key, check logs |
| WebSocket fails | Verify server running: `curl http://localhost:44380/api/health` |

## Import Test Data

```bash
# Obsidian vault
node scripts/import/obsidian.js ~/my-vault

# Wikipedia articles (network required)
node scripts/stress-test-wikipedia.js

# RSS feeds
node scripts/import-rss.js "https://example.com/feed.xml"
```

## Code Style

```bash
# Format all
cargo fmt
npx prettier --write src/

# Lint
cargo clippy -- -D warnings
npx eslint src/

# Type check
npx tsc --noEmit
```

## Commit Message Convention

```
<type>(<scope>): <subject>

<body>

<footer>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Example:
```
feat(embedding): add retry logic

Previously failed embeddings were silently dropped. Now retry up to 3 times
with exponential backoff before marking as failed.

Fixes #123
```

## Useful Resources

- **Architecture:** [CLAUDE.md](./CLAUDE.md)
- **Setup:** [DEV_SETUP.md](./DEV_SETUP.md)
- **Contributing:** [CONTRIBUTING.md](./CONTRIBUTING.md)
- **API Docs:** `docs/manual/api/`
- **Concepts:** `docs/manual/concepts/`
- **Discord:** [Join community](https://discord.gg/fT4vTERhz3)

## Performance Profiling

```bash
# Frontend flame graph (Chrome DevTools → Performance)
npm run tauri dev
# Record interaction in DevTools, analyze

# Backend with flamegraph
cargo install flamegraph
cargo flamegraph -p atomic-server -- serve --port 8080
# Open flamegraph.svg
```

## File Locations

| What | Where |
|------|-------|
| Business logic | `crates/atomic-core/src/` |
| HTTP API | `crates/atomic-server/src/routes/` |
| React components | `src/components/` |
| State management | `src/stores/` |
| React hooks | `src/hooks/` |
| Tests | `src/**/*.test.ts` or `crates/*/tests/` |
| Documentation | `docs/` |
| Database schemas | `crates/atomic-core/src/storage/migrations/` |
| Config | `Cargo.toml`, `package.json`, `vite.config.ts` |

## Environment Variables

```bash
# Frontend (.env.local)
VITE_ATOMIC_SERVER_URL=http://localhost:44380
VITE_BUILD_TARGET=desktop

# Rust (shell)
RUST_LOG=debug
RUST_BACKTRACE=1
ATOMIC_DATA_DIR=./data
ATOMIC_SERVER_PORT=8080
```

## MCP Development

```bash
# Build bridge
npm run build:mcp-bridge

# Test with Claude Desktop (macOS)
# Add to ~/Library/Application Support/Claude/claude_desktop_config.json:
{
  "mcpServers": {
    "atomic": {
      "command": "/path/to/atomic-mcp-bridge"
    }
  }
}
# Restart Claude Desktop

# Available MCP tools
semantic_search, read_atom, create_atom, ingest_url, update_atom
```

---

For detailed guides, see [DEV_SETUP.md](./DEV_SETUP.md) and [CONTRIBUTING.md](./CONTRIBUTING.md).
