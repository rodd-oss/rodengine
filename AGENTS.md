# AGENTS.md

## Commands
- **Build**: `bun run build` (runs vue-tsc typecheck + vite build)
- **Dev**: `bun run dev` (vite dev server on port 1420)
- **Typecheck**: `vue-tsc --noEmit`
- **Tauri dev**: `bun run tauri dev`
- **Tauri build**: `bun run tauri build`
- **Rust**: Use `cargo check`, `cargo build`, `cargo test` in src-tauri/

## Database Commands
- **Build database crate**: `cargo build -p ecsdb`
- **Test database**: `cargo test -p ecsdb`
- **Run benchmarks**: `cargo bench -p ecsdb`
- **Check formatting**: `cargo fmt --check -p ecsdb`
- **Lint**: `cargo clippy -p ecsdb -- -D warnings`
- **Generate docs**: `cargo doc -p ecsdb --no-deps`
- **Workspace build**: `cargo build --workspace`
- **Workspace test**: `cargo test --workspace`

## Code Style
- **Vue**: Use `<script setup lang="ts">` syntax, composition API
- **TypeScript**: Strict mode enabled, no unused locals/parameters
- **Imports**: Use ES6 imports, Vue composables from 'vue'
- **Tauri**: Use `invoke()` from @tauri-apps/api/core for Rust commands
- **Rust**: Standard rustfmt, use `#[tauri::command]` for exposed functions
- **Naming**: camelCase for JS/TS, snake_case for Rust
- **Error handling**: Use try/catch in TS, Result<> in Rust