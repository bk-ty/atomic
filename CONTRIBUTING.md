# Contributing to Atomic

Thank you for your interest in contributing to Atomic! This document outlines how to contribute code, documentation, or design.

## Code of Conduct

Be respectful, collaborative, and constructive. Atomic serves developers and knowledge enthusiasts worldwide.

## Before You Start

1. **Read** [DEV_SETUP.md](./DEV_SETUP.md) to set up your environment
2. **Understand** [CLAUDE.md](./CLAUDE.md) for architecture and design principles
3. **Check** [issues](https://github.com/kenforthewin/atomic/issues) to avoid duplicate work
4. **Join** the [Discord](https://discord.gg/fT4vTERhz3) to discuss major changes

## Development Workflow

### 1. Fork and Clone

```bash
git clone https://github.com/YOUR_USERNAME/atomic.git
cd atomic
git remote add upstream https://github.com/kenforthewin/atomic.git
```

### 2. Create a Feature Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/issue-123
```

Use descriptive names: `feature/semantic-edges-ui`, `fix/double-embedding-race`, `docs/ai-provider-setup`

### 3. Make Your Changes

#### Rust (Backend)

- Make changes in `crates/atomic-core/` (business logic) or `crates/atomic-server/` (HTTP layer)
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format code
- Test with `cargo test -p <crate>`

#### TypeScript (Frontend)

- Make changes in `src/` for the React app
- Follow [Airbnb JavaScript Style Guide](https://github.com/airbnb/javascript)
- Use ESLint (runs on `npm test`)
- Test with `npm test`

#### Documentation

- Update relevant `.md` files
- If adding a new feature, document it in `docs/manual/`
- Include code examples where helpful

### 4. Write Tests

**Always test your changes.** Examples:

**Rust unit test:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_deduplication() {
        // Arrange
        let core = setup_test_core();
        let atom_id = "test-123";

        // Act
        core.embed_atoms(vec![atom_id]).unwrap();
        core.embed_atoms(vec![atom_id]).unwrap();

        // Assert
        let atom = core.get_atom(atom_id).unwrap();
        assert_eq!(atom.embedding_status, EmbeddingStatus::Embedded);
    }
}
```

**TypeScript unit test (Vitest):**
```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { useAtomsStore } from '../stores/atoms';

describe('useAtomsStore', () => {
  beforeEach(() => {
    useAtomsStore.setState({
      atoms: [],
      selectedAtomId: null,
    });
  });

  it('adds an atom', () => {
    const store = useAtomsStore.getState();
    const atom = { id: '1', title: 'Test', content: 'Test content' };

    store.addAtom(atom);

    expect(useAtomsStore.getState().atoms).toContainEqual(atom);
  });
});
```

**TypeScript integration test (Playwright):**
```typescript
import { test, expect } from '@playwright/test';

test('user can create and search an atom', async ({ page }) => {
  await page.goto('http://localhost:1420');

  // Create atom
  await page.click('[data-testid="create-atom"]');
  await page.fill('[data-testid="atom-title"]', 'My Atom');
  await page.fill('[data-testid="atom-content"]', 'Some content');
  await page.click('[data-testid="save-atom"]');

  // Verify it appears in search
  await page.fill('[data-testid="search-input"]', 'My Atom');
  await expect(page.locator('text=My Atom')).toBeVisible();
});
```

### 5. Verify Your Changes

Run all checks before opening a PR:

```bash
# Rust
cargo fmt
cargo clippy -- -D warnings
cargo test --all

# TypeScript
npm test
npx tsc --noEmit
npx eslint src/

# Integration (if applicable)
npm run dev:server &
npm run test:e2e
```

### 6. Commit with Clear Messages

Format: `<type>(<scope>): <subject>`

Examples:
```
feat(embedding): add retry logic for failed embeddings
fix(ui): prevent double-submission of atom form
docs(setup): add Ollama installation steps
refactor(core): simplify similarity calculation
test(canvas): add position persistence tests
```

Use the conventional commit format so automation can process your changes.

**Commit message guidelines:**
- Use imperative mood: "Fix bug" not "Fixed bug"
- First line is 50 characters or less
- Reference issues: "Fixes #123", "Relates to #456"
- Explain *why*, not *what* (code shows what; commit shows why)

Example full commit:
```
feat(semantic-edges): compute similarity on chunk creation

Previously, semantic edges were computed asynchronously after all chunks
were created. This delayed discoverability and made testing harder. Now
compute similarity scores during chunk insertion using the embedding
pipeline's callback system.

- Create semantic_edges table entry immediately after chunk embedding
- Update graph in real-time as embeddings complete
- Add integration test verifying edges appear in canvas within 5s

Fixes #456
```

### 7. Push and Open a Pull Request

```bash
git push origin feature/your-feature-name
```

Then open a PR on GitHub. Fill in the PR template:

```markdown
## Description
Brief summary of what this PR does and why.

## Type of Change
- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change
- [ ] Documentation update

## Testing
Describe how you tested this. Include:
- New test cases
- Manual testing steps
- Affected areas

## Checklist
- [ ] Code follows project style
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] No console errors or warnings
- [ ] Commit messages are clear
```

### 8. Respond to Feedback

Address review comments in follow-up commits (don't squash during review).

```bash
# Make requested changes
git add .
git commit -m "Address review feedback: improve error handling"
git push
```

After approval, we'll squash or merge based on the PR's history.

## Common Contribution Types

### Bug Fix

**Steps:**
1. Create issue describing the bug (if one doesn't exist)
2. Branch: `git checkout -b fix/issue-123`
3. Add a test that reproduces the bug
4. Fix the bug (test should now pass)
5. Open PR with "Fixes #123" in the description

### New Feature

**Steps:**
1. **Discuss first:** Open an issue or ask on Discord — get feedback before coding
2. **Design:** Create a design document if UI/UX changes are involved
3. **Branch:** `git checkout -b feature/your-feature`
4. **Implement:** Follow the workflow above
5. **Document:** Add to `docs/manual/` if user-facing
6. **Test thoroughly:** Edge cases, error paths, integration

### Documentation

**Steps:**
1. Branch: `git checkout -b docs/topic`
2. Edit or create `.md` files in `docs/`
3. Use clear headings, code examples, and cross-links
4. Test links and formatting
5. Open PR

### Performance Improvement

**Steps:**
1. Profile to identify the bottleneck (use `flamegraph` or Chrome DevTools)
2. Document the performance impact (before/after metrics)
3. Implement the optimization
4. Benchmark to verify improvement
5. Open PR with benchmark results

## Architecture Guidelines

### When Adding a New Feature

1. **Keep `atomic-core` agnostic:** All business logic lives in the core, with no knowledge of Tauri, REST, or MCP
2. **Use callbacks for events:** Don't expose channels or async structures; use `Fn(Event)` closures
3. **Store per-database state in the database:** Not in `AtomicCore` or static `Mutex`
4. **Document invariants:** What preconditions must be true? What does this operation guarantee?

### When Modifying the API

1. **Backward-compatible first:** Can you deprecate instead of removing?
2. **Update all clients:** Frontend, mobile, MCP bridge, Obsidian plugin
3. **Test with real data:** Use the stress test scripts to validate on large datasets

### When Changing the Database Schema

1. **Create a migration:** Add a migration file to `crates/atomic-core/src/storage/migrations/`
2. **Test on existing data:** Run on a real database with thousands of atoms
3. **Document the change:** Add comments explaining what changed and why
4. **Update tests:** Ensure old and new schema paths work during migration

## Code Review Checklist

When reviewing (or being reviewed), check:

- **Correctness:** Does it solve the stated problem? Are edge cases handled?
- **Simplicity:** Is there a simpler approach? Does it add unnecessary complexity?
- **Performance:** Any N+1 queries, unbounded allocations, or potential deadlocks?
- **Safety:** Proper error handling? Unsafe code justified?
- **Maintainability:** Would a new developer understand this in 6 months?
- **Tests:** Are error paths covered? Any integration tests?
- **Documentation:** Are non-obvious decisions explained?

## Project Values

1. **Simplicity over cleverness:** The easiest implementation is often the best
2. **Explicit over implicit:** Better to be verbose than surprising
3. **Correctness over speed:** Bugs cost more than slow code
4. **User-driven:** Changes should solve real problems, not hypothetical ones
5. **Open source first:** Solutions benefit the whole community

## Getting Help

- **Architecture questions?** Ask in the [Discord](https://discord.gg/fT4vTERhz3) or open a discussion issue
- **Stuck on a PR?** Leave a comment asking for help
- **Want feedback early?** Open a draft PR and ask for guidance
- **Confused about the code?** That's a doc improvement — let us know what's unclear

## Release Process

Releases follow semantic versioning. After your PR is merged:
- Maintainers will periodically cut releases
- Your contribution will be included in the next release notes
- You'll see your name in the CHANGELOG

## License

By contributing, you agree that your contributions are licensed under the [MIT License](./LICENSE).

---

**Thank you for contributing to Atomic!** 🙌
