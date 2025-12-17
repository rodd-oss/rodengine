# Agent Instructions

## Build/Lint/Test Commands

- **JavaScript/TypeScript**: `bun test` (all), `bun test <file>` (single), `bun test --test-name-pattern <pattern>`. **Lint**: `bunx eslint .`. **Type check**: `bunx tsc --noEmit`.
- **Rust**: `cargo test` (all), `cargo test <test_name>` (single). **Lint**: `cargo clippy --all-targets -- -D warnings`. **Format**: `cargo fmt --all -- --check` (check), `cargo fmt --all` (apply).
- **Pre-commit**: `lint-staged` (prettier + eslint) + Rust checks (`cargo check`, `cargo clippy`, `cargo fmt --check`).

## Code Style & Notes

- **Imports**: ES modules with `verbatimModuleSyntax`. Avoid `import type` confusion.
- **Formatting**: Prettier default settings. ESLint includes prettier.
- **Types**: Strict TypeScript (`strict: true`). Explicit types, no `any`. No non-null assertions (`!`).
- **Naming**: camelCase variables/functions, PascalCase classes/types/interfaces.
- **Error handling**: `try/catch` for async errors, `Result` type for Rust.
- **Workspace**: Monorepo with `apps/*` and `packages/*`. Respect package boundaries.
- **Runtime**: Bun runtime/package manager.

## Linting/Formatting Policy

**IMPORTANT**: Never bypass/suppress linting rules. Fix underlying issues:

- **Rust**: Fix clippy warnings. Use `assert!(value)` not `assert_eq!(value, true/false)`.
- **JavaScript**: Fix ESLint warnings. No `// eslint-disable` comments.
- **Never** use `#[allow(clippy::...)]` attributes.

## btca

CLI tool for codebase questions. Triggers: user says "use btca" or agent hesitates.
Run: `btca ask -t <tech> -q "<question>"`. Available tech: svelte, tailwindcss, rust.
