# Test Plan for Task RL-2: Store Relations in Schema and Enforce Referential Integrity on Record Delete

## Context

- In‑memory relational database with `Vec<u8>` storage and tight packing.
- Relations defined between tables via `Relation` struct (source/destination tables, field mapping).
- Referential integrity must be enforced when a record is deleted.
- Atomic operations via `ArcSwap`; concurrent reads/writes possible.

## Assumptions

- `Relation` struct exists (from task RL‑1) with fields: `id`, `source_table`, `destination_table`, `source_field`, `destination_field`.
- Schema can store multiple relations.
- Default integrity rule is **RESTRICT** (prevent deletion if referenced). Cascading deletes may be a future extension.
- Record deletion is atomic (all‑or‑nothing).

---

## 1. Relation Storage Tests

**test_relation_add_and_retrieve**  
Verifies that a relation can be added to the schema and retrieved correctly.

- Setup: Two tables (`users`, `posts`) with appropriate fields.
- Action: Add relation `users.id` → `posts.user_id`.
- Assert: Relation is present in schema; fields match.

**test_duplicate_relation_rejected**  
Adding a relation with identical source/destination/field mapping should be rejected.

- Expect: Error (`RelationAlreadyExists`).

**test_relation_with_nonexistent_table**  
Attempt to create a relation referencing a table that does not exist.

- Expect: Error (`TableNotFound`).

**test_relation_with_nonexistent_field**  
Attempt to create a relation referencing a field that does not exist in the source/destination table.

- Expect: Error (`FieldNotFound`).

**test_remove_relation**  
Remove a previously added relation; ensure it no longer appears in schema.

- Action: Delete relation by its ID.
- Assert: Relation list no longer contains it.

**test_self_referential_relation**  
A table can reference itself (e.g., `employees.manager_id` → `employees.id`).

- Expect: Success; integrity checks work for same‑table references.

---

## 2. Referential Integrity – RESTRICT (Prevent Deletion)

**test_delete_record_without_references**  
Deleting a record that is **not** referenced by any other record should succeed.

- Setup: `users` table with one record; no relations from `posts`.
- Action: Delete the user record.
- Assert: Deletion succeeds; record count decrements.

**test_delete_record_with_references**  
Deleting a record that **is** referenced by another record should fail.

- Setup: `users` record referenced by a `posts` record via relation.
- Action: Attempt to delete the user.
- Expect: Error (`ReferentialIntegrityViolation`) with details about referencing table/record.

**test_delete_referencing_record**  
Deleting a record that references another record should succeed (orphaning allowed).

- Setup: `posts` record references a `users` record.
- Action: Delete the `posts` record.
- Assert: `posts` record removed; `users` record unchanged.

**test_multiple_references**  
A record referenced by multiple other records blocks deletion until all references are removed.

- Setup: One `user` referenced by three `posts`.
- Action: Attempt to delete user.
- Expect: Failure; error lists all referencing records.

**test_reference_across_multiple_relations**  
Record may be referenced via different relations (e.g., `users.id` → `posts.author_id` and `users.id` → `comments.user_id`).

- Expect: Deletion blocked until **all** referencing records in **all** relations are removed.

---

## 3. Edge Cases & Concurrency

**test_concurrent_delete_and_reference_add**  
While one transaction attempts to delete a record, another transaction adds a new reference to that record.

- Expect: Atomicity – either the delete fails (if reference added before commit) or succeeds (if reference added after).
- Use `ArcSwap` load/store to simulate concurrent access.

**test_delete_record_in_self_referential_table**  
Record A references record B within the same table; deleting B should be blocked while A exists.

- Expect: RESTRICT error.

**test_cyclic_references**  
Table A references table B, and table B references table A (direct cycle).

- Note: Cycle detection may be out of scope; but deletion of any record in the cycle should be blocked if referenced.

**test_empty_tables**  
Relations exist but tables have zero records. Deleting a non‑existent record should behave as normal (no‑op or error).

**test_relation_after_table_deletion**  
If a table is deleted, all relations involving that table should be automatically removed from schema.

- Expect: Schema clean‑up; subsequent referential checks ignore removed relations.

**test_integrity_across_buffer_swap**  
When storage buffer is swapped via `ArcSwap`, referential checks must use the **latest** buffer.

- Setup: Reference added after buffer swap.
- Action: Delete referenced record.
- Expect: Deletion blocked (reference exists in new buffer).

---

## 4. Error Reporting & Performance

**test_error_message_includes_context**  
Referential‑integrity error should indicate which relation and referencing record caused the violation.

- Assert: Error contains table name, record ID, and relation ID.

**test_performance_no_linear_scan**  
Integrity check should not scan entire referencing table on every delete (requires index).

- Can be verified by measuring operation time with large tables.
- Expect: O(1) or O(log n) lookup.

---

## Implementation Notes

- Use `cargo test` with `--test‑threads=1` for concurrency tests.
- Mock `ArcSwap` with `std::sync::Arc` for unit tests.
- Relation storage can be a `HashMap<RelationId, Relation>` inside `Schema`.
- Referential check should be performed **before** any buffer modification; failure must leave storage unchanged.
- All tests should be placed in `packages/db/src/relation/tests.rs` (or similar).
