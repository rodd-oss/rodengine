# Test Plan for task_mi_1: Create Rust crate under packages/db with proper Cargo.toml

## 1. Crate Structure Tests

- **test_crate_directory_exists**: Verifies `packages/db/` directory is created
- **test_cargo_toml_exists**: Verifies `packages/db/Cargo.toml` file exists
- **test_src_directory_exists**: Verifies `packages/db/src/` directory exists
- **test_lib_rs_exists**: Verifies `packages/db/src/lib.rs` exists (empty library)

## 2. Cargo.toml Configuration Tests

- **test_cargo_toml_valid_toml**: Verifies Cargo.toml is valid TOML syntax
- **test_package_name_correct**: Verifies `[package] name = "db"` matches directory
- **test_package_version**: Verifies version is `0.1.0` (initial)
- **test_edition_2021**: Verifies `edition = "2021"` for modern Rust
- **test_authors_present**: Verifies authors field includes project maintainers
- **test_description_present**: Verifies description matches database purpose
- **test_license_present**: Verifies license field exists (e.g., MIT/Apache-2.0)

## 3. Workspace Integration Tests

- **test_workspace_membership**: Verifies crate is listed in root `Cargo.toml` workspace members
- **test_workspace_path_correct**: Verifies workspace path is `"packages/db"` (not relative)
- **test_no_dependencies_conflict**: Verifies no conflicting dependencies with workspace root

## 4. Feature Flags Tests

- **test_default_features_empty**: Verifies no default features initially
- **test_feature_flags_structure**: Verifies feature flags section exists for future expansion
- **test_optional_dependencies**: Verifies optional dependencies can be added (e.g., `serde`, `arc-swap`)

## 5. Build Configuration Tests

- **test_build_succeeds**: Verifies `cargo build` succeeds without errors
- **test_test_succeeds**: Verifies `cargo test` runs (empty test suite initially)
- **test_check_succeeds**: Verifies `cargo check` passes
- **test_clippy_clean**: Verifies `cargo clippy` produces no warnings

## 6. Edge Cases & Validation Tests

- **test_no_duplicate_workspace_members**: Verifies crate not listed multiple times in workspace
- **test_dependency_version_constraints**: Verifies dependency versions use caret (`^`) for compatibility
- **test_no_dev_dependencies_initially**: Verifies `[dev-dependencies]` section optional initially
- **test_build_targets**: Verifies crate builds as library (`lib`) not binary
- **test_crate_type**: Verifies `crate-type = ["lib"]` in Cargo.toml

## 7. Integration with Other Languages

- **test_no_typescript_conflict**: Verifies no conflict with Bun/TypeScript workspace
- **test_go_workspace_independent**: Verifies Go workspace (`go.work`) unaffected
- **test_monorepo_structure_preserved**: Verifies existing `apps/*` and `packages/*` structure maintained

## Assertions & Expected Behaviors

- All file/directory paths exist and are accessible
- Cargo.toml parses without syntax errors
- Workspace membership validated via `cargo metadata`
- Build produces `target/debug/libdb.rlib`
- Test suite runs (even if empty) with `cargo test`
- No warnings from `cargo clippy -- -D warnings`
- Crate can be imported by other workspace members
