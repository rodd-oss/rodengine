# Test Plan for task_mi_2: Add crate to workspace root Cargo.toml

## Overview

Unit tests for verifying correct addition of database crate to workspace root Cargo.toml.

## Test Cases

### 1. **workspace_members_inclusion**

**Description**: Verify database crate is correctly added to workspace members.
**Verifies**: `"packages/db"` appears in `members` array of root Cargo.toml.
**Edge Cases**:

- Workspace already has other members
- Duplicate member entries
- Relative vs absolute paths
  **Assertions**:
- `members` array contains `"packages/db"`
- No duplicate entries for same crate
- Path is valid relative path from workspace root

### 2. **cargo_workspace_validation**

**Description**: Test that `cargo metadata` recognizes workspace configuration.
**Verifies**: Cargo can parse workspace and identify all members.
**Edge Cases**:

- Malformed TOML syntax
- Missing closing brackets
- Invalid member paths
  **Assertions**:
- `cargo metadata --format-version 1` succeeds
- Output includes `packages/db` in workspace members
- No parsing errors

### 3. **crate_dependency_resolution**

**Description**: Verify other workspace members can depend on database crate.
**Verifies**: Cross-crate dependencies work within workspace.
**Edge Cases**:

- Version conflicts with external dependencies
- Feature flags compatibility
- Build script dependencies
  **Assertions**:
- Can add `db = { path = "../packages/db" }` to another crate's Cargo.toml
- `cargo check` succeeds for dependent crate
- No duplicate crate versions in dependency graph

### 4. **workspace_inheritance**

**Description**: Test workspace-level dependencies and configurations are inherited.
**Verifies**: Database crate inherits workspace defaults (rust version, profiles, etc.).
**Edge Cases**:

- Workspace with custom profiles
- Override of workspace defaults in crate
- Conflicting toolchain requirements
  **Assertions**:
- Crate uses workspace Rust edition if specified
- Workspace dependency versions are respected
- Build profiles (dev, release) are inherited

### 5. **build_and_test_workspace**

**Description**: Verify entire workspace builds and tests successfully.
**Verifies**: Integration of new crate doesn't break existing workspace.
**Edge Cases**:

- Circular dependencies
- Missing dev-dependencies for tests
- Platform-specific code
  **Assertions**:
- `cargo build --workspace` succeeds
- `cargo test --workspace` runs all tests
- No compilation warnings (or acceptable warnings)

### 6. **crate_metadata_consistency**

**Description**: Ensure crate metadata (name, version) matches workspace expectations.
**Verifies**: Crate name and version are valid and consistent.
**Edge Cases**:

- Invalid crate names (special characters, reserved words)
- Semantic versioning violations
- Missing required fields in Cargo.toml
  **Assertions**:
- Crate name follows Rust conventions (snake_case)
- Version follows semver (x.y.z)
- All required Cargo.toml fields present

### 7. **feature_isolation**

**Description**: Test crate features don't conflict with workspace features.
**Verifies**: Feature flags work independently and don't cause conflicts.
**Edge Cases**:

- Feature name collisions
- Optional dependencies with features
- Default feature overrides
  **Assertions**:
- Can enable/disable crate features independently
- Workspace features don't affect crate unnecessarily
- Feature combinations compile correctly

### 8. **path_resolution_edge_cases**

**Description**: Test various path configurations and symlinks.
**Verifies**: Workspace handles different path representations correctly.
**Edge Cases**:

- Symlinked package directories
- Absolute vs relative paths
- Nested workspace configurations
- Path traversal attacks (../ escaping)
  **Assertions**:
- Symlinks to crate directory work
- Paths resolve to actual directories
- No directory traversal vulnerabilities

## Edge Cases to Consider

1. **Version conflicts** between workspace dependencies and crate dependencies
2. **Circular dependencies** within workspace members
3. **Missing Cargo.toml** in crate directory
4. **Invalid TOML syntax** causing parse failures
5. **Workspace with no default members** configuration
6. **Platform-specific dependencies** causing cross-compilation issues
7. **Build script failures** propagating to workspace level
8. **Feature unification** conflicts across workspace

## Test Infrastructure

- Use `cargo metadata` for workspace validation
- Parse Cargo.toml with `toml` crate for assertions
- Mock filesystem for edge case testing
- Integration tests that actually run `cargo` commands
- Unit tests for path validation and TOML manipulation logic
