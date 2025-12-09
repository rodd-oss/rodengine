# Phase 0: Project Setup
**Estimated Time:** Week 0

## Overview
Prepare development environment, establish repository structure, set up build systems, continuous integration, and basic tooling.

## Prerequisites
- Rust toolchain (rustc, cargo) >= 1.70
- Node.js & bun (for frontend)
- Git

## Subtasks

### 0.1 Repository Structure
- Create workspace Cargo.toml at root to manage multiple crates (ecsdb, ecsdb_client, dashboard?)
- Decide on crate layout: separate `ecsdb` library crate inside `src-tauri/` or sibling directory
- Create directory structure per architecture doc (`src/schema/`, `src/storage/`, etc.)
- Set up `.gitignore` for Rust, Node, build artifacts

### 0.2 Dependency Management
- Add dependencies to `ecsdb/Cargo.toml`: tokio, serde, toml, bincode, thiserror, dashmap, parking_lot, uuid, zstd, bytes, etc.
- Add dev‑dependencies: criterion, proptest, tokio‑test, insta, pretty_assertions
- Frontend: Ensure Tauri 2 and Vue 3 dependencies are up‑to‑date

### 0.3 Build & Script Configuration
- Update `AGENTS.md` with commands for building, testing, benchmarking the database
- Create `justfile` or `Makefile` with common tasks (build, test, bench, doc, lint)
- Configure `rustfmt.toml` and `clippy.toml` for consistent code style
- Set up `pre‑commit` hooks (format, lint, test)

### 0.4 Continuous Integration
- GitHub Actions workflow: test on Linux, macOS, Windows
- CI steps: `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo doc`
- Benchmark CI (optional): run criterion benchmarks, track regressions
- Coverage reporting (tarpaulin, codecov)

### 0.5 Development Environment
- Editor configuration (`.editorconfig`, `.vscode/settings.json`)
- Recommended extensions (rust‑analyzer, Tauri, Vue)
- Debug configuration (launch.json for VS Code)
- Docker dev container (optional)

### 0.6 Initial Documentation
- Update `README.md` with project vision, building instructions, quick start
- Create `CONTRIBUTING.md` with development workflow
- Add `CODE_OF_CONDUCT.md`
- License file (MIT/Apache 2.0)

### 0.7 Example Schema & Test Data
- Create `examples/simple_schema.toml` with entities, transform, health components
- Create `examples/basic_usage.rs` showing database creation, entity insertion, query
- Create integration test that loads example schema and performs basic operations

### 0.8 Tooling Checks
- Verify that `cargo build`, `cargo test`, `cargo run` work
- Verify Tauri dev server starts (`bun run tauri dev`)
- Run `cargo fmt` and `cargo clippy` on existing code

## Acceptance Criteria
1. Workspace builds without errors (`cargo build --workspace`)
2. All tests pass (`cargo test --workspace`)
3. Code formatting passes (`cargo fmt --check`)
4. Linting passes (`cargo clippy --workspace -- -D warnings`)
5. CI pipeline passes on first commit
6. Documentation is accessible and up‑to‑date
7. Example schema and basic usage example work

## Output Artifacts
- Workspace Cargo.toml
- Updated AGENTS.md with database‑specific commands
- GitHub Actions workflow file
- Example schema and usage code
- Pre‑commit hooks configuration

## Notes
- Keep setup simple; avoid over‑engineering tooling early
- Ensure all developers can get started with `git clone` + `cargo build`
- Choose permissive licensing to encourage adoption
