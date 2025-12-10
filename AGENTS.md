# AGENTS.md

## Commands
- **Build**: `bun run build` (vue-tsc + vite build)
- **Dev**: `bun run dev` (vite dev server on port 1420)
- **Typecheck**: `vue-tsc --noEmit`
- **Tauri dev**: `bun run tauri dev`
- **Tauri build**: `bun run tauri build`
- **Rust**: Use `cargo check`, `cargo build`, `cargo test` in src-tauri/
- **Single test**: `cargo test -p ecsdb --test integration` or `cargo test test_name`
- **Workspace build**: `cargo build --workspace`
- **Workspace test**: `cargo test --workspace`
- **Format check**: `cargo fmt --all --check`
- **Lint**: `cargo clippy --workspace -- -D warnings`
- **Benchmarks**: `cargo bench --workspace`
- **Docs**: `cargo doc --workspace --no-deps`

## Database Commands
- **Build**: `cargo build -p ecsdb`
- **Test**: `cargo test -p ecsdb`
- **Bench**: `cargo bench -p ecsdb`
- **Format**: `cargo fmt --check -p ecsdb`
- **Lint**: `cargo clippy -p ecsdb -- -D warnings`
- **Doc**: `cargo doc -p ecsdb --no-deps`

## Code Style
- **Vue**: `<script setup lang="ts">`, composition API, ES6 imports
- **TypeScript**: Strict mode, no unused locals/parameters, camelCase
- **Tauri**: Use `invoke()` from @tauri-apps/api/core for Rust commands
- **Rust**: snake_case, `#[tauri::command]` for exposed functions, rustfmt max_width=100, tab_spaces=4
- **Imports**: ES6 imports, workspace dependencies from Cargo.toml
- **Indentation**: 2 spaces for JS/TS/Vue, 4 spaces for Rust/TOML
- **Error handling**: try/catch in TS, Result<> in Rust, propagate with `?`
- **EditorConfig**: Follow .editorconfig rules