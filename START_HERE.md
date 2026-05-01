# 🚀 START HERE — Welcome to Atomic Development

**Your development environment is ready. Pick one of these.**

## Option 1: Desktop App (Recommended)

```bash
npm run tauri dev
```

Hot-reloading desktop app with full browser DevTools. Best for UI/full-stack work.

## Option 2: Web Development

```bash
npm run dev:server      # Terminal 1
npm run dev:web         # Terminal 2 → http://localhost:1420
```

## Option 3: Backend Only

```bash
cargo run -p atomic-server -- serve --port 8080
```

## Option 4: Frontend Only (Existing Server)

```bash
npm run dev             # Connects to localhost:44380
```

---

## Next Steps

1. **Run one of the commands above**
2. **Explore the UI** and make a small change
3. **Read [GETTING_STARTED.md](./GETTING_STARTED.md)** for your development path
4. **Check [QUICK_REF.md](./QUICK_REF.md)** for common commands
5. **Follow [CONTRIBUTING.md](./CONTRIBUTING.md)** for your first contribution

## Essential Documentation

| Document | Read When |
|----------|-----------|
| [ENVIRONMENT_READY.md](./ENVIRONMENT_READY.md) | Want to see what we set up |
| [GETTING_STARTED.md](./GETTING_STARTED.md) | Need orientation |
| [DEV_SETUP.md](./DEV_SETUP.md) | Need detailed instructions |
| [QUICK_REF.md](./QUICK_REF.md) | Need to find a command |
| [CONTRIBUTING.md](./CONTRIBUTING.md) | Ready to contribute |
| [CLAUDE.md](./CLAUDE.md) | Want to understand architecture |

## Common Commands

```bash
npm test                # Run tests
make check              # Type check + lint
make fmt                # Format code
npm run tauri dev       # Start desktop dev
cargo test              # Rust tests
```

---

**That's it. Pick an option above and start coding.** 🎉

Need help? Check [DEV_SETUP.md](./DEV_SETUP.md#troubleshooting) or join [Discord](https://discord.gg/fT4vTERhz3).
