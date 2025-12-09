.PHONY: build test bench doc lint fmt clean

# Workspace commands
build:
	cargo build --workspace

test:
	cargo test --workspace

bench:
	cargo bench --workspace

doc:
	cargo doc --workspace --no-deps

lint:
	cargo clippy --workspace -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clean:
	cargo clean

# Database-specific commands
db-build:
	cargo build -p ecsdb

db-test:
	cargo test -p ecsdb

db-bench:
	cargo bench -p ecsdb

db-doc:
	cargo doc -p ecsdb --no-deps

db-lint:
	cargo clippy -p ecsdb -- -D warnings

# Tauri-specific commands
tauri-dev:
	bun run tauri dev

tauri-build:
	bun run tauri build

# Frontend commands
frontend-dev:
	bun run dev

frontend-build:
	bun run build

frontend-typecheck:
	vue-tsc --noEmit