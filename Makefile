.PHONY: help setup dev dev-desktop dev-desktop-fast dev-server dev-web test test-watch check fmt lint clean build build-desktop build-server build-web build-mobile db-reset db-drop import-obsidian

# Default target
help:
	@echo "Atomic Development Commands"
	@echo ""
	@echo "Setup:"
	@echo "  make setup              Install all dependencies"
	@echo ""
	@echo "Development:"
	@echo "  make dev                Frontend only (existing server)"
	@echo "  make dev-desktop        Desktop app with Tauri (full release build)"
	@echo "  make dev-desktop-fast   Desktop app, incremental debug build, uses release data"
	@echo "  make dev-server         Headless server (port 8080)"
	@echo "  make dev-web            Web frontend (port 1420)"
	@echo ""
	@echo "Testing & Checking:"
	@echo "  make test               Run all tests"
	@echo "  make test-watch         Watch mode (frontend only)"
	@echo "  make check              Type/lint checks"
	@echo "  make fmt                Format all code"
	@echo "  make lint               Run linters"
	@echo ""
	@echo "Database:"
	@echo "  make db-reset           Reset SQLite database"
	@echo "  make db-drop            Drop all databases"
	@echo "  make import-obsidian    Import Obsidian vault"
	@echo ""
	@echo "Building:"
	@echo "  make build              Build all targets"
	@echo "  make build-desktop      Build desktop app"
	@echo "  make build-server       Build server binary"
	@echo "  make build-web          Build web bundle"
	@echo "  make build-mobile       Build mobile apps"
	@echo "  make build-mcp          Build MCP bridge"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean              Clean build artifacts"
	@echo "  make cache-clear        Clear npm/cargo caches"

# Setup
setup:
	@echo "Installing dependencies..."
	npm install
	cargo build --workspace
	@echo "✓ Setup complete"

# Development
ARCH := $(shell rustc -vV 2>/dev/null | sed -n 's/host: //p')

dev:
	npm run dev

# Fast desktop dev: incremental debug build + copy sidecar + launch Tauri.
# Tauri resolves data dir from app identifier (com.atomic.app) — same as release.
dev-desktop-fast:
	@echo "Building atomic-server (debug, incremental)..."
	cargo build -p atomic-server
	@echo "Installing sidecar ($(ARCH))..."
	cp target/debug/atomic-server src-tauri/binaries/atomic-server-$(ARCH)
	@echo "Launching Tauri dev (data: ~/Library/Application Support/com.atomic.app)"
	npx tauri dev --config src-tauri/tauri.dev-fast.conf.json

dev-desktop:
	npm run tauri dev
dev-server:
	npm run dev:server

dev-web:
	npm run dev:web

# Testing
test:
	cargo test --all
	npm test

test-watch:
	npm test -- --watch

# Checking
check:
	cargo check --all
	npx tsc --noEmit

fmt:
	cargo fmt
	npx prettier --write src/

lint:
	cargo clippy -- -D warnings
	npx eslint src/ --fix

# Database
db-reset:
	npm run db:reset

db-reset-tags:
	npm run db:reset-tags

db-reset-chunks:
	npm run db:reset-chunks

db-drop:
	npm run db:drop

import-obsidian:
	@read -p "Enter Obsidian vault path: " vault_path; \
	node scripts/import/obsidian.js "$$vault_path"

# Building
build: build-server build-web build-desktop
	@echo "✓ All builds complete"

build-desktop:
	npm run tauri build

build-server:
	npm run build:server

build-web:
	npm run build:web

build-mobile:
	npm run build:mobile

build-mobile-ios:
	npm run build:mobile:ios

build-mobile-android:
	npm run build:mobile:android

build-mcp:
	npm run build:mcp-bridge

# Maintenance
clean:
	cargo clean
	rm -rf dist-web target node_modules/.cache

cache-clear:
	cargo clean
	npm cache clean --force

# Pre-commit checks (for CI/hooks)
pre-commit: fmt lint test check
	@echo "✓ All pre-commit checks passed"
