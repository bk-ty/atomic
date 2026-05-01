# Getting Started with Atomic Development

Welcome! This document helps you get up and running with Atomic's development environment.

## The 60-Second Setup

```bash
# Clone (if you haven't already)
git clone https://github.com/kenforthewin/atomic.git
cd atomic

# Install and verify
bash scripts/setup-dev.sh

# Start developing
npm run tauri dev              # Desktop app with hot reload
```

That's it. Everything else is optional.

## Choose Your Path

### 🖥️ Desktop App Development (Recommended)

Best for: UI/UX work, full-stack features, user testing.

```bash
npm run tauri dev
```

Gives you:
- Hot reload for frontend (React)
- Automatic backend recompilation
- Built-in sidecar server
- Full browser DevTools

### 🌐 Web Development (Two Terminals)

Best for: Web-specific features, progressive web app work.

**Terminal 1:**
```bash
npm run dev:server
```

**Terminal 2:**
```bash
npm run dev:web
```

Then open http://localhost:1420.

### ⚙️ Backend-Only Development

Best for: API design, business logic, database work.

```bash
cargo run -p atomic-server -- serve --port 8080
```

Then use the REST API or open a web/mobile client pointing to `http://localhost:8080`.

### 📱 Frontend-Only Development

Best for: working against an existing server.

```bash
npm run dev
```

Connects to http://localhost:44380 by default. Configure in `.env.local`.

## Documentation

| Document | Purpose |
|----------|---------|
| [**DEV_SETUP.md**](./DEV_SETUP.md) | Complete setup guide with all options and troubleshooting |
| [**QUICK_REF.md**](./QUICK_REF.md) | Quick command reference — keep this open while coding |
| [**CONTRIBUTING.md**](./CONTRIBUTING.md) | Guidelines for contributions (tests, commits, PRs) |
| [**CLAUDE.md**](./CLAUDE.md) | Architecture deep-dive and design principles |
| [**Makefile**](./Makefile) | Convenient `make` commands for common tasks |

## Common Commands

```bash
# Development
npm run tauri dev              # Desktop with hot reload
npm run dev:server            # Headless server
npm run dev:web               # Web frontend

# Testing
cargo test                    # All Rust tests
npm test                      # All TypeScript tests
npm test -- --watch           # Watch mode (frontend)

# Code quality
cargo fmt && cargo clippy      # Format and lint Rust
npx prettier --write src/      # Format TypeScript
npx tsc --noEmit               # Type check

# Database
npm run db:reset               # Fresh SQLite database
npm run db:reset-tags          # Reset tags only
npm run db:reset-chunks        # Reset embeddings only

# Building
npm run tauri build            # Desktop production build
npm run build:server           # Server binary
npm run build:web              # Web bundle

# Help
make help                      # List all make targets
```

## Architecture at a Glance

```
📦 atomic-core
   └─ All business logic (CRUD, search, embeddings, wiki, chat)
      ✓ No framework dependencies
      ✓ Transport-agnostic
      ✓ Fully tested

📦 atomic-server
   └─ HTTP REST API + WebSocket + MCP
      ✓ ~78 routes
      ✓ Event streaming via broadcast
      ✓ Headless or as Tauri sidecar

⚛️ React Frontend
   └─ Web UI (three-panel layout)
      ✓ Works desktop, web, mobile
      ✓ State via Zustand
      ✓ Transport abstraction (same code runs anywhere)

🖥️ Tauri Desktop App
   └─ Wraps React + sidecar server
      ✓ Uses HttpTransport like web
      ✓ Native file/dialogs/menus
      ✓ Auto-updates

📱 Mobile (iOS/Android)
   └─ Capacitor wrapper around React
      ✓ Shared frontend code
      ✓ Native plugins for APIs

🧩 Browser Extension
   └─ Capture and queue web content

🤖 MCP Bridge
   └─ Stdio-to-HTTP adapter for Claude Desktop
```

## Project Structure

```
atomic/
├── crates/atomic-core/        ← Core business logic (Rust)
├── crates/atomic-server/      ← HTTP server (Rust + actix-web)
├── src/                       ← React frontend (TypeScript)
├── src-tauri/                 ← Tauri desktop app shell
├── mobile/                    ← iOS (Capacitor) + Android
├── plugins/obsidian-plugin/   ← Obsidian plugin
├── extension/                 ← Browser extension
├── scripts/                   ← Import, build, utilities
├── docs/                      ← User documentation
├── DEV_SETUP.md               ← ⭐ Read this first
├── QUICK_REF.md               ← ⭐ Keep open while coding
├── CONTRIBUTING.md            ← ⭐ Read before PRs
└── Makefile                   ← Convenient shortcuts
```

## First Contribution

1. **Pick a task:** Browse [issues](https://github.com/kenforthewin/atomic/issues), or ask in [Discord](https://discord.gg/fT4vTERhz3)
2. **Create a branch:** `git checkout -b feature/your-feature`
3. **Make changes:** Follow code style (see [CONTRIBUTING.md](./CONTRIBUTING.md))
4. **Test:** `cargo test && npm test && npx tsc --noEmit`
5. **Commit:** Clear message with why, not just what
6. **Push & PR:** Include tests and description

See [CONTRIBUTING.md](./CONTRIBUTING.md) for detailed guidelines.

## Key Concepts

**Atoms:** Markdown notes with metadata (URLs, tags, timestamps). Automatically chunked, embedded, tagged, and linked.

**Tags:** Hierarchical tree organizing atoms. Auto-extracted by LLM into categories (Topics, People, Locations, etc.).

**Embeddings:** Vector representations generated by AI provider (OpenRouter/Ollama). Power semantic search and similarity.

**Wiki:** LLM-synthesized articles summarizing all atoms under a tag, with inline citations.

**Chat:** Agentic RAG interface scoped to specific tags. Searches knowledge base during conversation.

**Canvas:** Force-directed graph visualization where atoms are nodes, semantic similarity determines layout.

## Tools You'll Use

- **Rust:** Business logic and backend
- **React 18:** Frontend UI
- **Tauri v2:** Desktop app framework
- **SQLite + sqlite-vec:** Database + vector search
- **Zustand:** Frontend state management
- **Vite:** Frontend build and dev server
- **TypeScript:** Type-safe frontend code

## Getting Help

- **Questions?** Ask in [Discord](https://discord.gg/fT4vTERhz3)
- **Stuck?** Check [DEV_SETUP.md troubleshooting section](./DEV_SETUP.md#troubleshooting)
- **Architecture unclear?** Read [CLAUDE.md](./CLAUDE.md)
- **Contribution workflow?** See [CONTRIBUTING.md](./CONTRIBUTING.md)
- **Command reference?** Check [QUICK_REF.md](./QUICK_REF.md)

## What's Next?

1. **Run the app:** `npm run tauri dev` and explore
2. **Read architecture:** [CLAUDE.md](./CLAUDE.md) for the big picture
3. **Make a small fix:** Try a "good first issue" from GitHub
4. **Join the community:** [Discord](https://discord.gg/fT4vTERhz3)

---

**Happy coding!** 🚀
