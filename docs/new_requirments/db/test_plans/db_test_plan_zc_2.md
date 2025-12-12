# Test Plan for Iterator Over Table Records (task_zc_2)

## 1. Test Names & Descriptions

- `test_iter_empty_table` – Iterator over empty buffer yields no items.
- `test_iter_single_record` – Iterator over buffer with exactly one record yields that record.
- `test_iter_multiple_records` – Iterator yields all records in insertion order.
- `test_iter_double_ended` – Iterator supports `next_back` and traverses from both ends.
- `test_iter_size_hint` – Iterator’s `size_hint` matches actual remaining count.
- `test_iter_ref_lifetime` – References returned by iterator live at least as long as the iterator.
- `test_iter_concurrent_snapshot` – Iterator holds a snapshot of the buffer (ArcSwap load) and is unaffected by subsequent writes.
- `test_iter_record_size_alignment` – Iterator respects record size and never yields misaligned slices.
- `test_iter_out_of_bounds_safety` – Iterator stops cleanly when buffer length is not a multiple of record size (should panic or return None).
- `test_iter_zip_with_indices` – Pair iteration with record indices matches manual index calculation.

## 2. What each test verifies

- `empty` → `next()` returns `None` immediately.
- `single` → `next()` returns a reference to the sole record; second call returns `None`.
- `multiple` → Sequence of references matches inserted data (e.g., compare deserialized field values).
- `double_ended` → Forward and backward iteration meet in the middle; combined sequence equals original order.
- `size_hint` → `size_hint().0` equals exact remaining count; `size_hint().1` is `Some` of same value.
- `ref_lifetime` → Store reference in a variable and ensure it can be used after iterator advances (no use‑after‑free).
- `concurrent_snapshot` → Load buffer snapshot before write; iterator yields old records even after buffer is swapped.
- `record_size_alignment` → Each yielded slice length equals defined record size; slice start offset is aligned to record size.
- `out_of_bounds_safety` → If buffer length is not a multiple of record size, iteration panics (debug) or stops early (release) – verify expected behavior.
- `zip_with_indices` → Enumerate iterator yields `(usize, &[u8])` where index matches `i * record_size`.

## 3. Edge Cases to Consider

- **Empty table** – iterator must be safe and produce no items.
- **Single record** – ensure reference points to correct region of buffer.
- **Maximum capacity** – iterate over many records (e.g., fill buffer to capacity) without performance degradation.
- **Buffer snapshotting** – iterator must hold a strong reference to the Arc‑wrapped buffer to prevent deallocation.
- **Double‑ended iteration** – front and back iterators should not overlap or skip records.
- **Record size zero** – should be prohibited by schema validation; iterator could panic or loop forever.
- **Misaligned buffer length** – iterator must detect and stop/panic (debug assertion).
- **Concurrent modification** – iterator should see a consistent snapshot; writers may swap buffers concurrently.
- **Iterator `Send`/`Sync`** – verify iterator can be sent across threads (if designed for parallel iteration later).

## 4. Assertions & Expected Behaviors

- `assert_eq!(iter.next(), None)` for empty.
- `assert_eq!(iter.next().map(|r| r.as_ptr()), Some(expected_ptr))`.
- `assert_eq!(collected.len(), expected_count)`.
- `assert!(iter.size_hint().0 == remaining && iter.size_hint().1 == Some(remaining))`.
- `assert_eq!(front.next(), back.next_back())` for double‑ended meeting point.
- `assert!(std::mem::size_of_val(&*ref) > 0)` for lifetime safety.
- `assert_eq!(snapshot_iter.count(), pre_write_count)`.
- `assert!(slice.len() == RECORD_SIZE)`.
- `assert!(slice.as_ptr() as usize % RECORD_SIZE == 0)`.
- `assert!(iter.enumerate().all(|(i, r)| r.as_ptr() == buffer.as_ptr().add(i * RECORD_SIZE)))`.

## Implementation notes

Tests should be placed in the same module as the iterator implementation, using `#[cfg(test)]`. Use `cargo test` to run. Mock a minimal table buffer (Vec<u8>) and record size to isolate iterator logic from higher‑level schema concerns.
