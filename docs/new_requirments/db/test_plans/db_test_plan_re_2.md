# Test Plan for task_re_2: Validate snapshot integrity (checksum, version)

## 1. **Test: valid_snapshot_passes_validation**

**Description**: Verify that a correctly generated snapshot with proper checksum and version passes validation.
**Verifies**: Basic validation logic accepts valid snapshots.
**Edge cases**: None (positive test).
**Assertions**: `validate_snapshot()` returns `Ok(())` or `true`.

## 2. **Test: corrupted_checksum_fails_validation**

**Description**: Tamper with checksum bytes in snapshot header.
**Verifies**: Checksum validation detects data corruption.
**Edge cases**: Single-byte corruption, multi-byte corruption, zeroed checksum.
**Assertions**: `validate_snapshot()` returns `Err(SnapshotError::ChecksumMismatch)`.

## 3. **Test: invalid_version_fails_validation**

**Description**: Test with unsupported/outdated version numbers.
**Verifies**: Version compatibility checking.
**Edge cases**: Version 0 (uninitialized), version higher than supported, negative version (if signed).
**Assertions**: `validate_snapshot()` returns `Err(SnapshotError::UnsupportedVersion)`.

## 4. **Test: truncated_snapshot_fails_validation**

**Description**: Provide partial snapshot data (missing bytes).
**Verifies**: Length validation and bounds checking.
**Edge cases**: Empty file, missing header, missing data section, missing footer.
**Assertions**: `validate_snapshot()` returns `Err(SnapshotError::InvalidFormat)` or `Err(SnapshotError::UnexpectedEof)`.

## 5. **Test: malformed_header_fails_validation**

**Description**: Corrupt magic bytes or header structure.
**Verifies**: Header format validation.
**Edge cases**: Wrong magic bytes, incorrect header size, misaligned fields.
**Assertions**: `validate_snapshot()` returns `Err(SnapshotError::InvalidFormat)`.

## 6. **Test: checksum_covers_entire_payload**

**Description**: Modify data section while keeping header checksum unchanged.
**Verifies**: Checksum includes entire snapshot payload, not just header.
**Edge cases**: Modify first data byte, last data byte, middle of large dataset.
**Assertions**: `validate_snapshot()` returns `Err(SnapshotError::ChecksumMismatch)`.

## 7. **Test: version_migration_validation**

**Description**: Test with multiple supported versions (if versioning scheme exists).
**Verifies**: Backward/forward compatibility logic.
**Edge cases**: Version 1→2 migration, deprecated but still readable versions.
**Assertions**: Older supported versions pass validation; unsupported versions fail.

## 8. **Test: concurrent_modification_detection**

**Description**: Simulate snapshot being modified during read (if applicable).
**Verifies**: Atomic snapshot validation.
**Edge cases**: File size changes mid-read, checksum mismatch due to concurrent write.
**Assertions**: `validate_snapshot()` returns appropriate error (checksum mismatch or read error).

## 9. **Test: hardware_corruption_scenarios**

**Description**: Simulate bit flips at various positions.
**Verifies**: Robustness against hardware-level corruption.
**Edge cases**: Single-bit flip in checksum, single-bit flip in version field, single-bit flip in data.
**Assertions**: All corruption scenarios are detected (checksum mismatch or invalid format).

## 10. **Test: endianness_validation**

**Description**: Test with wrong byte order (if cross-platform support needed).
**Verifies**: Endianness handling in version/checksum fields.
**Edge cases**: Big-endian vs little-endian mismatch.
**Assertions**: Proper error for endianness mismatch or automatic detection.

## 11. **Test: performance_large_snapshots**

**Description**: Validate checksum on large snapshot (multi-GB scale).
**Verifies**: Checksum calculation performance and memory usage.
**Edge cases**: Streaming validation vs full-memory validation.
**Assertions**: Validation completes without OOM; performance scales linearly.

## 12. **Test: recovery_from_invalid_snapshot**

**Description**: Test error handling and recovery flow after validation failure.
**Verifies**: Clean error reporting and system state after validation failure.
**Edge cases**: Partial validation before failure, resource cleanup.
**Assertions**: System returns to clean state; appropriate error logged.

## Key Implementation Considerations

- Use cryptographic hash (SHA-256) or CRC32 for checksum depending on security vs performance needs
- Version field should include major.minor.patch semantics
- Header format: magic bytes (4-8 bytes), version (4 bytes), checksum (32 bytes for SHA-256), payload size (8 bytes)
- Validation should be atomic - either entire snapshot valid or invalid
- Consider memory-mapped I/O for large snapshots
