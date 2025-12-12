# Agent Instructions

## Build/Lint/Test Commands

- **JavaScript/TypeScript lint**: `bunx eslint .` (no npm script). Pre-commit hook runs `prettier --list-different` and `eslint`.
- **TypeScript type check**: `bunx tsc --noEmit`
- **Test**: `bun test` (Bun's built-in test runner). Single test: `bun test <file>` or `bun test --test-name-pattern <pattern>`
- **Build**: No build script yet. For workspaces, run `bun build` per package.
- **Rust**: `cargo test` (all tests), `cargo build`. Single test: `cargo test <test_name>`

## Code Style Guidelines

- **Imports**: ES modules with `verbatimModuleSyntax`. Avoid `import type` confusion.
- **Formatting**: Prettier with default settings (empty .prettierrc). ESLint config includes prettier.
- **Types**: Strict TypeScript (`strict: true`). Explicit types, no `any`. Non-null assertions (`!`) prohibited.
- **Naming**: camelCase variables/functions, PascalCase classes/types/interfaces.
- **Error handling**: `try/catch` for async errors, `Result` type for Rust.
- **Vue**: Vue 3 composition API; follow `eslint-plugin-vue` essential rules.
- **Workspace**: Monorepo with `apps/*` and `packages/*`. Respect package boundaries.

## Additional Notes

- Bun runtime/package manager. Husky pre-commit hook runs lint-staged.
- ESLint plugins: Vue, JSON, Markdown, CSS. Follow respective lint rules.
- No Cursor or Copilot rules present.
