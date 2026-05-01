#!/bin/bash
# Setup script for Atomic development environment
# Installs git hooks, verifies tools, and prepares the workspace

set -e

echo "🚀 Atomic Development Environment Setup"
echo ""

# Color codes
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check required tools
echo "📋 Checking prerequisites..."

check_tool() {
  if command -v "$1" &> /dev/null; then
    local version=$($1 --version 2>&1 | head -1)
    echo "  ✓ $1: $version"
  else
    echo "  ✗ $1: NOT FOUND"
    return 1
  fi
}

errors=0

check_tool node || errors=$((errors + 1))
check_tool npm || errors=$((errors + 1))
check_tool cargo || errors=$((errors + 1))
check_tool rustc || errors=$((errors + 1))
check_tool git || errors=$((errors + 1))

if [ $errors -gt 0 ]; then
  echo ""
  echo -e "${RED}❌ Missing required tools${NC}"
  echo "Please install: Node.js, Rust, and Git"
  echo "See DEV_SETUP.md for installation instructions"
  exit 1
fi

echo ""
echo "✓ All prerequisites found"

# Install dependencies
echo ""
echo "📦 Installing dependencies..."
npm install > /dev/null 2>&1
echo "  ✓ npm dependencies"

cargo check --workspace > /dev/null 2>&1
echo "  ✓ Rust workspace"

# Setup git hooks
echo ""
echo "🔗 Installing git hooks..."

if [ ! -d ".git/hooks" ]; then
  mkdir -p .git/hooks
fi

if [ -f "scripts/hooks/pre-commit" ]; then
  cp scripts/hooks/pre-commit .git/hooks/pre-commit
  chmod +x .git/hooks/pre-commit
  echo "  ✓ pre-commit hook installed"
fi

# Create .env.local if it doesn't exist
echo ""
echo "⚙️  Configuring environment..."

if [ ! -f ".env.local" ]; then
  cat > .env.local << 'EOF'
# Atomic Development Configuration

# Frontend
VITE_ATOMIC_SERVER_URL=http://localhost:44380
VITE_BUILD_TARGET=desktop

# Optional: Uncomment to point to a different server
# VITE_ATOMIC_SERVER_URL=http://localhost:8080

# Rust
# RUST_LOG=debug
EOF
  echo "  ✓ .env.local created"
else
  echo "  ℹ .env.local already exists"
fi

# Print summary
echo ""
echo "✅ Setup complete!"
echo ""
echo -e "${GREEN}Quick Start:${NC}"
echo "  Desktop app:     ${YELLOW}npm run tauri dev${NC}"
echo "  Server + Web:    ${YELLOW}npm run dev:server${NC} (terminal 1) + ${YELLOW}npm run dev:web${NC} (terminal 2)"
echo "  Frontend only:   ${YELLOW}npm run dev${NC}"
echo ""
echo -e "${GREEN}Useful Commands:${NC}"
echo "  make dev-desktop     # Desktop app with Tauri"
echo "  make test            # Run all tests"
echo "  make check           # Type/lint checks"
echo "  make fmt             # Format code"
echo "  make db-reset        # Reset database"
echo ""
echo "📖 For more information, see:"
echo "  - DEV_SETUP.md (detailed setup guide)"
echo "  - QUICK_REF.md (command reference)"
echo "  - CONTRIBUTING.md (contribution guidelines)"
echo ""
