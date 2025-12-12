# Agent Instructions

## Build/Lint/Test Commands

- **JavaScript/TypeScript lint**: `bunx eslint .` (no npm script). **Type check**: `bunx tsc --noEmit`. Pre-commit hook runs `prettier --list-different` and `eslint`.
- **Test**: `bun test` (Bun's built-in test runner). Single test: `bun test <file>` or `bun test --test-name-pattern <pattern>`
- **Build**: No build script yet. For workspaces, run `bun build` per package.
- **Rust**: `cargo test` (all tests), `cargo build`. Single test: `cargo test <test_name>`
- **Rust lint**: `cargo clippy --workspace -- -D warnings` (run clippy with all warnings as errors). **NEVER** use clippy allows (`-A`) or bypass linting - fix the underlying issues instead.
- **Rust format**: `cargo fmt -- --check` (check formatting), `cargo fmt` (apply formatting). **ALWAYS** fix formatting issues rather than bypassing them.
- **Go**: `go test ./...` (no Go code yet). Workspace defined in `go.work`.

## Code Style & Notes

- **Imports**: ES modules with `verbatimModuleSyntax`. Avoid `import type` confusion.
- **Formatting**: Prettier with default settings (empty .prettierrc). ESLint config includes prettier.
- **Types**: Strict TypeScript (`strict: true`). Explicit types, no `any`. Non-null assertions (`!`) prohibited. `noUncheckedIndexedAccess` enabled.
- **Naming**: camelCase variables/functions, PascalCase classes/types/interfaces.
- **Error handling**: `try/catch` for async errors, `Result` type for Rust.
- **Vue**: Vue 3 composition API; follow `eslint-plugin-vue` essential rules.
- **Workspace**: Monorepo with `apps/*` and `packages/*`. Respect package boundaries. Workspace configs: package.json (Bun), Cargo.toml (Rust), go.work (Go).
- **Runtime**: Bun runtime/package manager. ESLint plugins: Vue, JSON, Markdown, CSS.
- **Pre-commit**: Husky hook runs lint-staged (`prettier --list-different` then `eslint`).
- **No Cursor/Copilot rules** present.
- **Rust/Go workspaces** exist but no code yet.
- **Repository structure**: See `REPO_STRUCTURE.txt` for detailed project layout.
- **Database project**: See `docs/new_requirments/db/` for in-memory database requirements and TDD test plans.

## Linting/Formatting Policy

**IMPORTANT**: Never bypass or suppress linting/formatting rules. Always fix the underlying issues:

### Rust Clippy Rules

- **Fix clippy warnings** instead of allowing them with `-A` flags
- Common fixes needed:
  - Replace `assert_eq!(value, true)` with `assert!(value)`
  - Replace `assert_eq!(value, false)` with `assert!(!value)`
  - Use `std::f32::consts::PI` instead of `3.14159`
  - Use `std::f64::consts::E` instead of `2.71828`
  - Fix approximate constant warnings by using proper constants
  - Replace manual modulo operations with `.is_multiple_of()`
  - Remove unnecessary casts

### JavaScript/ESLint Rules

- **Fix ESLint warnings** instead of disabling rules
- Use proper imports and exports
- Fix formatting issues with Prettier

### General Guidelines

- **Never** use `// eslint-disable` comments
- **Never** use `#[allow(clippy::...)]` attributes
- **Never** modify tool configurations to bypass rules
- **Always** address the root cause of linting failures
- **When unsure** about a lint rule, research best practices before fixing
