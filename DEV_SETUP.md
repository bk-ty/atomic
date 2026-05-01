# Local Development Setup Guide

Welcome to the Atomic development environment. This guide covers:
- Setting up for local development
- Running the application (desktop, server, or web)
- Understanding the codebase structure
- Common development tasks
- Debugging and troubleshooting
- Contributing best practices

## Prerequisites

**Required:**
- Node.js 22+ ([nodejs.org](https://nodejs.org))
- Rust toolchain ([rustup.rs](https://rustup.rs))
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Linux: GCC, pkg-config, sqlite3-dev
- Windows: Visual Studio Build Tools or full Visual Studio

**Optional (for desktop app):**
- Tauri v2 dependencies ([setup guide](https://v2.tauri.app/start/prerequisites/))

**Optional (for mobile):**
- iOS: Xcode 15+
- Android: Android Studio + JDK 17+

## Quick Start

### 1. Clone and Install Dependencies

```bash
git clone https://github.com/kenforthewin/atomic.git
cd atomic
npm install
cargo check
```

This installs npm dependencies and verifies the Rust workspace compiles.

### 2. Choose Your Development Path

#### Desktop App (Recommended for UI Development)
```bash
npm run tauri dev
```

Launches the Tauri dev environment with:
- Hot reload for React frontend
- Automatic backend recompilation
- Built-in server sidecar
- Full debugging tools

**First run:** You'll be prompted to configure an AI provider (OpenRouter, Ollama, or compatible).

#### Headless Server + Web Frontend

Terminal 1 — Start the server:
```bash
npm run dev:server
```

Terminal 2 — Start the web frontend:
```bash
npm run dev:web
```

Then open `http://localhost:1420`. The frontend communicates with the server at `http://localhost:44380` (configurable via `ATOMIC_SERVER_PORT`).

#### Frontend Only (Requires Running Server)

If you already have a local server or cloud instance:
```bash
npm run dev
```

The frontend loads from `http://localhost:44380` by default. Configure the server URL in `.env.local`:
```
VITE_ATOMIC_SERVER_URL=http://localhost:44380
```

#### Standalone Server (Rust Only)

```bash
cargo run -p atomic-server -- --data-dir ./data serve --port 8080
```

Create an API token:
```bash
cargo run -p atomic-server -- --data-dir ./data token create --name "dev"
```

Then use the token to access the REST API at `http://localhost:8080`.

## Project Structure

```
atomic/
├── crates/
│   ├── atomic-core/           # Core business logic (no framework deps)
│   ├── atomic-server/         # HTTP/WebSocket/MCP server (actix-web)
│   ├── mcp-bridge/            # stdio-to-HTTP MCP bridge
│   ├── atomic-cloud/          # Managed hosting control plane
│   └── atomic-bench/          # Benchmarking suite
├── src-tauri/                 # Tauri desktop app shell
├── src/                       # React frontend (TypeScript)
├── mobile/                    # iOS (Capacitor) and Android apps
├── extension/                 # Chrome/Chromium browser extension
├── plugins/
│   ├── discord/               # Discord bot integration
│   └── obsidian-plugin/       # Obsidian plugin
├── scripts/                   # Build, import, and utility scripts
├── databases/                 # Local development databases
├── docs/                      # Documentation
└── package.json, Cargo.toml   # Workspace manifests
```

### Key Directories

| Directory | Purpose | Watch For |
|-----------|---------|-----------|
| `crates/atomic-core/src/` | All business logic: CRUD, search, embeddings, wiki, chat | Domain model changes, storage layer |
| `crates/atomic-server/src/routes/` | REST API handlers (~78 routes) | API contract changes, new endpoints |
| `src/components/` | React UI components (atoms, wiki, chat, canvas, settings) | UI/UX changes, component interfaces |
| `src/stores/` | Zustand state management (atoms, tags, wiki, chat, ui, settings) | State shape, dispatch logic |
| `src/hooks/` | React hooks (transport, embedding progress, keyboard, etc.) | Event subscriptions, side effects |
| `src-tauri/src/` | Tauri IPC bridge, Apple Notes import | Desktop-specific logic |

## Common Development Tasks

### Running Tests

**All tests:**
```bash
cargo test                       # All Rust tests
npm test                         # All TypeScript/Vitest tests
```

**Specific crate/package:**
```bash
cargo test -p atomic-core        # Just atomic-core
npm test -- api.test.ts          # Just api.test.ts
```

**Watch mode (frontend):**
```bash
npm test -- --watch
```

**With coverage:**
```bash
npm test -- --coverage
```

### Type Checking

```bash
npx tsc --noEmit                 # Frontend types
cargo check                      # Rust types
```

### Database Management

**Reset development database:**
```bash
npm run db:reset                 # SQLite: full reset, preserves config
npm run db:reset:pg              # PostgreSQL: drop volume and recreate
```

**Reset specific tables:**
```bash
npm run db:reset-tags            # Clear tags, re-trigger auto-tagging
npm run db:reset-chunks          # Clear chunks/embeddings, re-embed
```

**Query the database directly:**
```bash
sqlite3 databases/default.db "SELECT id, title, embedding_status FROM atoms LIMIT 10;"
sqlite3 databases/registry.db "SELECT key, value FROM settings LIMIT 10;"
```

### Importing Test Data

Import markdown files from an Obsidian vault:
```bash
node scripts/import/obsidian.js ~/my-vault
```

Stress test with Wikipedia articles (requires network):
```bash
node scripts/stress-test-wikipedia.js
```

Import RSS feeds:
```bash
node scripts/import-rss.js "https://example.com/feed.xml"
```

### Building for Production

**Desktop app:**
```bash
npm run tauri build              # macOS/Linux/Windows binaries in src-tauri/target/release/bundle/
```

**Server + Web frontend:**
```bash
npm run build:server             # Binary: target/release/atomic-server
npm run build:web                # Static bundle: dist-web/
```

**Mobile apps:**
```bash
npm run build:mobile             # iOS + Android production builds
npm run build:mobile:ios         # Just iOS
npm run build:mobile:android     # Just Android
```

### MCP Bridge Development

The MCP bridge is a stdio-to-HTTP adapter for connecting local Atomic to Claude Desktop and other MCP clients.

**Build the bridge:**
```bash
npm run build:mcp-bridge         # Binary: target/release/atomic-mcp-bridge
```

**Test with Claude Desktop** (macOS):

1. Find your bridge binary path (build result or `/Applications/Atomic.app/...`)
2. Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "atomic": {
      "command": "/path/to/atomic-mcp-bridge"
    }
  }
}
```

3. Restart Claude Desktop. You should see "Atomic" in the MCP tools list.

### Obsidian Plugin Development

**Build the plugin:**
```bash
cd plugins/obsidian-plugin
npm run dev                      # Watch mode
npm run build                    # Production bundle
```

**Install in Obsidian:**

1. Create test vault
2. In settings, enable "Restricted mode" → Off
3. Copy the built bundle to `.obsidian/plugins/atomic/`
4. Enable the "Atomic" plugin in the Community Plugins list

### Docker Development

**Run the full stack locally:**
```bash
docker compose up -d
# Server at http://localhost:8080 (behind nginx)
# Web UI at http://localhost:8080
```

**Build from source:**
```bash
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build
```

**Database inspection in Docker:**
```bash
docker compose exec server sqlite3 /data/atomic.db "SELECT COUNT(*) FROM atoms;"
```

## Debugging

### Frontend Debugging

**React DevTools** (in Tauri):
- Open DevTools: Right-click → Inspect Element
- Or: Press `Ctrl+Shift+I` (Linux/Windows) or `Cmd+Option+I` (macOS)

**Console logging:**
```typescript
console.log('Debug:', value);
import { getLogger } from '@atomic/logger';  // If available
```

**Network inspection:**
- DevTools → Network tab
- Watch HTTP requests to the server
- WebSocket frame details in WS tab

### Backend Debugging

**Rust backtrace:**
```bash
RUST_BACKTRACE=1 cargo run -p atomic-server -- serve --port 8080
```

**Detailed logging:**
```bash
RUST_LOG=debug cargo run -p atomic-server -- serve --port 8080
# Or specific modules:
RUST_LOG=atomic_core::embedding=debug cargo run -p atomic-server -- serve --port 8080
```

**Database queries:**
Use `sqlite3` CLI to inspect database state during development.

### Tauri App Debugging

**Console in DevTools:**
- Tauri logs appear in the browser DevTools console
- Rust `println!` and `eprintln!` appear in the terminal

**Hot reload issues:**
If changes don't appear:
1. Hard refresh: `Cmd+Shift+R` (macOS) or `Ctrl+Shift+R` (Linux/Windows)
2. Clear browser cache: DevTools → Application → Storage → Clear site data
3. Restart: `npm run tauri dev`

### Performance Profiling

**Frontend:**
- Chrome DevTools → Performance tab → Record
- Run interactions, stop recording, analyze flame graph

**Backend:**
- Use [`flamegraph` crate](https://www.brendangregg.com/flamegraphs.html) or `cargo flamegraph`
- Profile under realistic load: `npm run stress:wikipedia`

## Development Conventions

### Rust Code Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Run `cargo clippy` to catch common mistakes

**Check and format all workspace crates:**
```bash
cargo fmt
cargo clippy -- -D warnings
```

**Before committing:**
```bash
cargo test
cargo fmt
cargo clippy
```

### TypeScript Code Style

- ESLint enforces rules (run via Vite)
- Use `prettier` for formatting (via VSCode or manually)
- Prefer explicit types over implicit `any`

**Check and format:**
```bash
npx tsc --noEmit
npm test
```

### Commit Guidelines

- **Atomic commits:** One logical change per commit
- **Clear messages:** "Fix X" is vague; "Prevent double-embedding when atom has status=pending" is clear
- **Link issues:** Include issue number if applicable
- **Test coverage:** New features should include tests

Example:
```
Fix double-embedding of recently-imported atoms

When atoms are bulk-imported, the pipeline may attempt to embed the same 
atom twice if an embedding status check races with the import completion 
handler. Lock the atom for the duration of the import transaction.

Fixes #1234
Tests: integration_tests.rs::test_bulk_import_single_pass
```

## Contributing

1. **Set up development environment** (this guide)
2. **Create a branch:** `git checkout -b feature/your-feature`
3. **Make changes:** Follow the conventions above
4. **Test locally:** `cargo test && npm test`
5. **Verify types:** `cargo check && npx tsc --noEmit`
6. **Format code:** `cargo fmt && prettier --write src/`
7. **Open a pull request** with clear description
8. **Address review feedback** in follow-up commits

### Code Review Focus Areas

- **Correctness:** Does the code do what it claims?
- **Simplicity:** Is there a simpler approach?
- **Performance:** Any N+1 queries, unbounded allocations, or unnecessary clones?
- **Maintainability:** Will the next person understand this?
- **Testing:** Are error cases covered?

## Environment Variables

**.env.local** (frontend):
```env
VITE_ATOMIC_SERVER_URL=http://localhost:44380  # Server base URL
VITE_BUILD_TARGET=desktop                       # "desktop" or "web"
```

**Rust (via environment):**
```bash
RUST_LOG=debug                                  # Logging level
RUST_BACKTRACE=1                                # Include backtraces
ATOMIC_DATA_DIR=./data                          # Data directory
ATOMIC_SERVER_PORT=8080                         # Server port
ATOMIC_DB_NAME=fresh                            # Fresh database (Tauri dev)
ATOMIC_PROVIDER=ollama                          # AI provider (openrouter/ollama)
```

## Troubleshooting

### "sqlite-vec extension not available"

**macOS/Linux:**
The project builds sqlite-vec from source. Ensure you have:
- GCC/Clang
- pkg-config
- Development headers for SQLite

```bash
brew install sqlite pkg-config  # macOS
apt install build-essential libsqlite3-dev  # Ubuntu
```

Then `cargo clean && cargo build`.

### "Node modules not found"

```bash
npm install
npm run postinstall               # Apply patches
```

### Tauri app won't start

```bash
# Clear cache and rebuild
rm -rf src-tauri/target
npm run tauri dev
```

If it persists, check [Tauri troubleshooting](https://v2.tauri.app/faq/).

### Server won't start on port 8080

Port may be in use:
```bash
lsof -i :8080                    # Check what's using the port
npm run dev:server -- --port 9000  # Use different port
```

### Embedding not progressing

1. Check AI provider configuration: Settings → AI Provider
2. Check logs: `RUST_LOG=debug cargo run -p atomic-server -- serve --port 8080`
3. Verify API key is valid and has quota
4. Check database: `sqlite3 databases/default.db "SELECT COUNT(*) FROM atoms WHERE embedding_status='pending';"`

### WebSocket connection fails

**In browser console:**
```javascript
// Check if server is accessible
fetch('http://localhost:44380/api/health')
  .then(r => r.json())
  .then(console.log)
  .catch(console.error);
```

Ensure server is running and CORS is configured correctly.

### Sidebar scrolls randomly when expanding tag

Tag accordion clicks scroll the sidebar to random positions.

**Fix:** See [Tag Accordion Scroll Fix](./docs/manual/guides/tag-accordion-scroll-fix.md) for implementation details. Root cause is stale virtualizer measurements during tree expansion.

## Next Steps

- **Read** [AGENTS.md](./AGENTS.md) for AI-powered development workflows
- **Review** [CLAUDE.md](./CLAUDE.md) for architecture deep-dive
- **Check** [crates/atomic-core/README.md](./crates/atomic-core/README.md) for core API docs
- **Join** the [Discord community](https://discord.gg/fT4vTERhz3)

Happy coding!
