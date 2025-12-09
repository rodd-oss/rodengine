# Contributing to ECSDb

Thank you for your interest in contributing to ECSDb! This document outlines the development workflow and standards.

## Development Workflow

1. **Fork & Clone**: Fork the repository and clone your fork.
2. **Create a Branch**: Use descriptive branch names (`feat/add-thing`, `fix/issue-123`).
3. **Make Changes**: Follow the code style guidelines below.
4. **Run Tests**: Ensure all tests pass with `cargo test --workspace`.
5. **Check Formatting**: Run `cargo fmt --all` and `cargo clippy --workspace -- -D warnings`.
6. **Commit**: Write clear commit messages following [Conventional Commits](https://www.conventionalcommits.org/).
7. **Push & Open PR**: Push your branch and open a pull request against the `main` branch.

## Code Style

### Rust
- Use `rustfmt` with the provided `rustfmt.toml` configuration.
- Follow Clippy lints (run `cargo clippy --workspace -- -D warnings`).
- Use `snake_case` for functions/variables, `PascalCase` for types.
- Document public APIs with doc comments (`///`).
- Prefer explicit error handling with `Result` and `thiserror`.

### TypeScript/Vue
- Use `<script setup lang="ts">` syntax with Composition API.
- Follow ESLint and TypeScript strict mode.
- Use `camelCase` for variables/functions, `PascalCase` for components.

## Testing
- Write unit tests for new functionality in the same module.
- Integration tests go in `tests/` directory.
- Aim for >80% test coverage (measured via `tarpaulin`).

## Documentation
- Update `README.md` if user-facing changes.
- Update inline documentation for public APIs.
- Update architecture docs in `docs/` if system design changes.

## Issue Reporting
- Use GitHub Issues for bug reports and feature requests.
- Include steps to reproduce, expected vs actual behavior, environment details.

## Pull Request Review
- PRs require at least one maintainer approval.
- All CI checks must pass.
- Keep PRs focused; split large changes into smaller PRs.

## License
By contributing, you agree that your contributions will be licensed under the project's MIT OR Apache-2.0 license.