# Agent Instructions

## Build/Lint/Test Commands

- **JavaScript/TypeScript lint**: `bunx eslint .` (no npm script). **Type check**: `bunx tsc --noEmit`. Pre-commit hook runs `prettier --list-different` and `eslint`.
- **Test**: `bun test` (Bun's built-in test runner). Single test: `bun test <file>` or `bun test --test-name-pattern <pattern>`
- **Build**: No build script yet. For workspaces, run `bun build` per package.
- **Rust**: `cargo test` (all tests), `cargo build`. Single test: `cargo test <test_name>`
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
