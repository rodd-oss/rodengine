# Agent Instructions

## Build/Lint/Test Commands

- **JavaScript/TypeScript**: `bun test` (all tests), `bun test <file>` (single test), `bun test --test-name-pattern <pattern>`. **Lint**: `bunx eslint .`. **Type check**: `bunx tsc --noEmit`.
- **Rust**: `cargo test` (all tests), `cargo test <test_name>` (single test). **Lint**: `cargo clippy --all-targets -- -D warnings`. **Format**: `cargo fmt --all -- --check` (check), `cargo fmt --all` (apply).
- **Pre-commit**: Runs `lint-staged` (prettier + eslint) plus Rust checks (`cargo check`, `cargo clippy`, `cargo fmt --check`).

## Code Style & Notes

- **Imports**: ES modules with `verbatimModuleSyntax`. Avoid `import type` confusion.
- **Formatting**: Prettier with default settings. ESLint config includes prettier.
- **Types**: Strict TypeScript (`strict: true`). Explicit types, no `any`. Non-null assertions (`!`) prohibited.
- **Naming**: camelCase variables/functions, PascalCase classes/types/interfaces.
- **Error handling**: `try/catch` for async errors, `Result` type for Rust.
- **Workspace**: Monorepo with `apps/*` and `packages/*`. Respect package boundaries.
- **Runtime**: Bun runtime/package manager. ESLint plugins: Vue, JSON, Markdown, CSS.
- **No Cursor/Copilot rules** present.

## Linting/Formatting Policy

**IMPORTANT**: Never bypass or suppress linting/formatting rules. Always fix the underlying issues:

- **Rust**: Fix clippy warnings instead of allowing them. Replace `assert_eq!(value, true/false)` with `assert!(value)`/`assert!(!value)`. Use proper constants.
- **JavaScript**: Fix ESLint warnings instead of disabling rules. Use proper imports and exports.
- **Never** use `// eslint-disable` comments or `#[allow(clippy::...)]` attributes.
- **Always** address the root cause of linting failures.

## btca

Btca is a cli tool to ask your codebase/docs some questions.

Triggers:

- user says "use btca"
- ai agent hesitates if function, method, property etc. exists

Run:

- `btca ask -t <tech> -q "<question>"`

Available `<tech>`: svelte, tailwindcss, rust
