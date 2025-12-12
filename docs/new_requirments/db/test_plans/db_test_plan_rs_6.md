# Test Plan for DELETE /relation/{id} (task_rs_6)

## 1. Basic Functionality Tests

- **test_delete_existing_relation**: Verify successful deletion of an existing relation

  - Creates two tables and a relation between them
  - Calls DELETE /relation/{id} with valid relation ID
  - Asserts: Returns 204 No Content, relation removed from schema
  - Verifies: Relation no longer accessible via GET endpoints

- **test_delete_relation_removes_from_catalog**: Verify relation removed from database catalog
  - Creates multiple relations
  - Deletes one specific relation
  - Asserts: Deleted relation absent from schema listing
  - Verifies: Other relations remain intact

## 2. Error Handling Tests

- **test_delete_nonexistent_relation**: Attempt to delete non-existent relation ID

  - Calls DELETE /relation/{invalid_id}
  - Asserts: Returns 404 Not Found
  - Verifies: No changes to schema or other relations

- **test_delete_relation_invalid_id_format**: Test with malformed ID
  - Calls DELETE /relation/{invalid_format} (non-numeric, empty string)
  - Asserts: Returns 400 Bad Request
  - Verifies: Clear error message about invalid ID format

## 3. Concurrency Tests

- **test_concurrent_delete_relation**: Multiple threads attempting to delete same relation

  - Creates relation, spawns multiple threads to delete it concurrently
  - Asserts: Only one deletion succeeds (returns 204)
  - Verifies: Other attempts return 404 (relation already deleted)
  - Edge case: Race condition handling with ArcSwap buffer

- **test_delete_relation_while_read_operations**: Delete during active reads
  - Starts read operations on related tables
  - Concurrently deletes the relation
  - Asserts: Read operations complete without panic using old buffer
  - Verifies: New reads after deletion fail appropriately

## 4. Referential Integrity Tests

- **test_delete_relation_with_existing_references**: Relation with existing record references

  - Creates tables with records linked via relation
  - Attempts to delete relation
  - Asserts: Returns 409 Conflict or similar error
  - Verifies: Relation persists, error indicates referential integrity violation
  - Edge case: Should relation deletion cascade? (Based on TRD: "enforce referential integrity on record delete")

- **test_delete_relation_after_clearing_references**: Delete after removing all references
  - Creates linked records, deletes them, then deletes relation
  - Asserts: Returns 204, relation removed successfully
  - Verifies: Schema updated correctly

## 5. Atomicity Tests

- **test_delete_relation_atomicity**: Verify all-or-nothing behavior

  - Mocks failure during relation deletion (e.g., disk write failure)
  - Asserts: Operation rolls back completely
  - Verifies: Schema remains unchanged, relation still accessible
  - Edge case: Partial failure during buffer swap with ArcSwap

- **test_delete_relation_transaction_isolation**: Relation deletion within transaction scope
  - Starts transaction, deletes relation, rolls back
  - Asserts: Relation restored after rollback
  - Verifies: GET endpoints show relation still exists

## 6. Schema Persistence Tests

- **test_delete_relation_persists_to_disk**: Verify deletion persists after restart

  - Creates relation, deletes it, triggers disk snapshot
  - Restarts database (simulated)
  - Asserts: Relation absent from loaded schema
  - Verifies: JSON schema file updated correctly

- **test_delete_relation_concurrent_with_schema_save**: Delete during background persistence
  - Triggers disk save, concurrently deletes relation
  - Asserts: Either operation succeeds, no data corruption
  - Verifies: Consistent state after both operations complete

## 7. Edge Cases

- **test_delete_last_relation**: Delete when it's the only relation

  - Creates single relation, deletes it
  - Asserts: Returns 204, empty relations list
  - Verifies: Schema still valid without relations

- **test_delete_relation_cross_table_validation**: Verify both source and destination tables unaffected

  - Creates relation between tables A and B
  - Deletes relation
  - Asserts: Tables A and B still exist with their records intact
  - Verifies: No side effects on unrelated tables

- **test_delete_relation_id_reuse_prevention**: Ensure deleted relation IDs aren't immediately reused
  - Creates and deletes relation, creates new relation
  - Asserts: New relation gets different ID
  - Verifies: No confusion between old and new relations

## 8. Performance Tests

- **test_delete_relation_performance_large_schema**: Delete relation in schema with 1000+ relations

  - Creates many relations, deletes one
  - Asserts: Operation completes within acceptable time
  - Verifies: Other relations remain accessible

- **test_delete_relation_memory_cleanup**: Verify buffer memory reclaimed
  - Creates large relation metadata, deletes it
  - Asserts: Memory usage decreases appropriately
  - Verifies: No memory leaks (use Rust's drop semantics)

## Assertions & Expected Behaviors:

- **HTTP Status Codes**: 204 (success), 404 (not found), 400 (bad request), 409 (conflict)
- **Atomic Guarantees**: No partial deletions visible to other threads
- **Concurrency Safety**: Lock-free reads continue during deletion
- **Schema Consistency**: JSON schema file matches in-memory state
- **Error Messages**: Clear, actionable error responses
- **Idempotency**: Multiple DELETE calls to same ID return same result (404 after first successful deletion)
