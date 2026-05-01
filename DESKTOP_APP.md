# Running the Atomic Desktop App

## Current Status

✅ **Server binary built** — `npm run build:server` completed successfully  
✅ **Dependencies installed** — All 925 npm packages  
✅ **Rust workspace ready** — atomic-core, atomic-server compiling  
✅ **Environment configured** — `.env.local` set up  

## Desktop App Development

### Option 1: Development Mode (Recommended)

```bash
npm run tauri dev
```

This command:
1. Builds the server sidecar
2. Starts the Vite dev server (hot reload)
3. Launches the Tauri dev environment
4. Opens the app window with browser DevTools

**First run takes 2-3 minutes** as it compiles everything.

Once running:
- Make code changes → automatic hot reload
- Edit Rust code → automatic recompilation
- Right-click in app → Inspect Element for DevTools
- App window stays open for debugging

### Option 2: Production Build

```bash
npm run tauri build
```

Creates optimized production binary in:
```
src-tauri/target/release/bundle/macos/Atomic.app
```

## Troubleshooting

### Issue: Build fails with "invalid value '1' for '--ci'"

**Solution:** This is a known issue with the Tauri CLI version. Use dev mode instead:
```bash
npm run tauri dev
```

### Issue: "atomic-server not found"

**Solution:** Build the server first:
```bash
npm run build:server
npm run tauri dev
```

### Issue: Port 1420 already in use

**Solution:** Kill existing process:
```bash
lsof -i :1420
kill -9 <PID>
npm run tauri dev
```

### Issue: App window won't open

**Solution:** Check the terminal for error messages. Common issues:
- Vite dev server failed to start
- Server sidecar failed to build
- Port conflict

Look for error messages and check [DEV_SETUP.md](./DEV_SETUP.md#troubleshooting).

## What to Do When App Opens

1. **On first launch:** Setup wizard for AI provider configuration
   - Choose OpenRouter, Ollama, or compatible provider
   - Enter API key
   - Select models (or auto-detect for Ollama)

2. **Create your first atom:**
   - Click "+" button
   - Write markdown content
   - Add tags
   - Save

3. **Explore features:**
   - Search atoms (semantic search)
   - View canvas (graph visualization)
   - Generate wiki articles
   - Try chat interface

## Development Workflow

### Making Changes

**Frontend (React/TypeScript):**
```bash
# Edit src/components/atoms/AtomCard.tsx
# → Hot reload automatically
```

**Backend (Rust):**
```bash
# Edit crates/atomic-core/src/lib.rs
# → Recompiles automatically
# → May require app restart
```

**Styles (Tailwind):**
```bash
# Edit src/index.css or component classes
# → Hot reload automatically
```

### Testing During Development

**Frontend tests (don't stop dev server):**
```bash
npm test              # In another terminal
npm test -- --watch   # Watch mode
```

**Backend tests:**
```bash
cargo test -p atomic-core    # In another terminal
```

### Debugging

**Browser DevTools:**
- Right-click in app → Inspect Element
- Or press: Cmd+Option+I (macOS)
- Console, Network, Performance tabs all work

**Rust logging:**
```bash
RUST_LOG=debug npm run tauri dev
```

**Database inspection:**
```bash
sqlite3 databases/default.db "SELECT COUNT(*) FROM atoms;"
```

## Commands Reference

```bash
# Development
npm run tauri dev              # Dev with hot reload
npm run tauri dev:fresh        # Fresh database
npm run tauri build            # Production build

# Server (if running separately)
npm run build:server           # Build server binary
cargo run -p atomic-server ... # Run directly

# Testing (while dev is running)
npm test                       # Frontend tests
cargo test                     # Backend tests

# Code quality
make check                     # Type check + lint
make fmt                       # Format code

# Database
npm run db:reset               # Fresh database
npm run db:reset-tags          # Reset tags only
```

## Next Steps

1. Run: `npm run tauri dev`
2. Wait for app window to open (2-3 minutes first time)
3. Go through setup wizard
4. Create your first atom
5. Make a test code change to see hot reload
6. Try each feature (search, canvas, wiki, chat)

## Architecture Reminder

```
Atomic Desktop App
├── React Frontend (TypeScript)
│   ├── Hot reload during dev
│   ├── Communicates via HTTP
│   └── Same code as web version
├── Tauri Wrapper (Rust)
│   ├── Native window/menus/dialogs
│   └── Launches server as sidecar
└── Atomic Server (Sidecar)
    ├── Built by npm run build:server
    ├── HTTP REST API
    └── WebSocket events
```

The app runs a local server in the background. You're not just running a web wrapper—it's a full local system with database, embeddings, search, etc.

## Performance Notes

- **First run:** 2-3 minutes (initial build)
- **Subsequent runs:** 10-30 seconds
- **Code changes:** 1-2 seconds for hot reload
- **Rust changes:** 5-30 seconds for recompilation

This is normal for Rust/Tauri development.

---

**Ready?** Run: `npm run tauri dev`
