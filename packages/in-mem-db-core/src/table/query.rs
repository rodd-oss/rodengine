//! Query-related methods for table operations.

use super::Table;
use crate::error::DbError;

impl Table {
    /// Queries records with simple field equality filters.
    ///
    /// # Arguments
    /// * `filters` - Field name to value mapping for equality filters
    /// * `limit` - Maximum number of records to return
    /// * `offset` - Number of records to skip
    ///
    /// # Returns
    /// `Result<Vec<usize>, DbError>` containing indices of matching records.
    ///
    /// # Performance
    /// - O(n) where n is number of records
    /// - Uses raw pointer comparisons for efficiency
    /// - Zero allocations per field comparison
    pub fn query_records(
        &self,
        filters: &std::collections::HashMap<String, Vec<u8>>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<usize>, DbError> {
        let buffer = self.buffer.load();
        let buffer_slice = buffer.as_slice();
        let record_count = self.record_count();

        // Pre-process filters to get field offsets and expected bytes
        let mut filter_specs = Vec::new();
        for (field_name, expected_bytes) in filters {
            if let Some(field) = self.get_field(field_name) {
                if expected_bytes.len() != field.size {
                    return Err(DbError::TypeMismatch {
                        expected: format!("{} bytes for field '{}'", field.size, field_name),
                        got: format!("{} bytes", expected_bytes.len()),
                    });
                }
                filter_specs.push((field.offset, expected_bytes.as_slice()));
            } else {
                return Err(DbError::FieldNotFound {
                    table: self.name.clone(),
                    field: field_name.clone(),
                });
            }
        }

        let mut matching_indices = Vec::new();
        let skip_count = offset.unwrap_or(0);
        let mut matched_total = 0;

        for record_index in 0..record_count {
            let record_offset =
                record_index
                    .checked_mul(self.record_size)
                    .ok_or(DbError::CapacityOverflow {
                        operation: "query operation",
                    })?;

            // Check all filters
            let mut matches_all = true;
            for &(field_offset, expected_bytes) in &filter_specs {
                let field_start =
                    record_offset
                        .checked_add(field_offset)
                        .ok_or(DbError::CapacityOverflow {
                            operation: "query operation",
                        })?;
                let field_end = field_start.checked_add(expected_bytes.len()).ok_or(
                    DbError::CapacityOverflow {
                        operation: "query operation",
                    },
                )?;

                if field_end > buffer_slice.len() {
                    return Err(DbError::InvalidOffset {
                        table: self.name.clone(),
                        offset: field_end,
                        max: buffer_slice.len().saturating_sub(expected_bytes.len()),
                    });
                }

                // Compare field bytes with expected bytes
                if &buffer_slice[field_start..field_end] != expected_bytes {
                    matches_all = false;
                    break;
                }
            }

            if matches_all {
                matched_total += 1;

                // Skip records based on offset
                if matched_total <= skip_count {
                    continue;
                }

                matching_indices.push(record_index);

                // Check limit
                if let Some(limit_val) = limit {
                    if matching_indices.len() >= limit_val {
                        break;
                    }
                }
            }
        }

        Ok(matching_indices)
    }

    /// Iterates over records in parallel using Rayon.
    ///
    /// # Arguments
    /// * `f` - Closure that processes each record chunk and returns a result
    ///
    /// # Returns
    /// `Result<Vec<R>, DbError>` containing the collected results from the closure.
    ///
    /// # Notes
    /// - Requires the `parallel` feature to be enabled.
    /// - The closure receives a byte slice for the record and its index.
    /// - The buffer is loaded once and shared across all parallel tasks.
    /// - Records are processed in contiguous chunks aligned to record_size.
    /// - Chunk boundaries are aligned to 64-byte cache lines to prevent false sharing.
    #[cfg(feature = "parallel")]
    pub fn par_iter_records<F, R>(&self, f: F) -> Result<Vec<R>, DbError>
    where
        F: Fn(&[u8], usize) -> R + Send + Sync,
        R: Send,
    {
        const CACHE_LINE_SIZE: usize = 64;

        /// Greatest common divisor using Euclidean algorithm.
        fn gcd(a: usize, b: usize) -> usize {
            let mut a = a;
            let mut b = b;
            while b != 0 {
                let temp = b;
                b = a % b;
                a = temp;
            }
            a
        }

        /// Least common multiple.
        fn lcm(a: usize, b: usize) -> usize {
            if a == 0 || b == 0 {
                0
            } else {
                a / gcd(a, b) * b
            }
        }

        let buffer = self.buffer.load();
        let buffer_slice = buffer.as_slice();
        let record_size = self.record_size;
        let total_bytes = buffer.len();

        if total_bytes == 0 {
            return Ok(Vec::new());
        }

        // Ensure buffer length is multiple of record size
        if total_bytes % record_size != 0 {
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset: total_bytes,
                max: total_bytes.saturating_sub(record_size),
            });
        }

        let record_count = total_bytes / record_size;
        let base_ptr = buffer.as_ptr() as usize;
        let aligned_offset = super::validation::align_offset(base_ptr, CACHE_LINE_SIZE) - base_ptr;

        // If aligned offset is beyond buffer length, we have no aligned region
        if aligned_offset >= total_bytes {
            // Entire buffer fits before first cache line boundary, process sequentially
            let results: Vec<R> = (0..record_count)
                .map(|idx| {
                    let start = idx * record_size;
                    let slice = &buffer_slice[start..start + record_size];
                    f(slice, idx)
                })
                .collect();
            return Ok(results);
        }

        // Ensure aligned offset is multiple of record size
        let aligned_record_offset = super::validation::align_offset(aligned_offset, record_size);
        if aligned_record_offset >= total_bytes {
            // Aligned region starts beyond buffer, process sequentially
            let results: Vec<R> = (0..record_count)
                .map(|idx| {
                    let start = idx * record_size;
                    let slice = &buffer_slice[start..start + record_size];
                    f(slice, idx)
                })
                .collect();
            return Ok(results);
        }

        // Process prefix records (before aligned region) sequentially
        let prefix_record_count = aligned_record_offset / record_size;
        let mut results: Vec<R> = Vec::with_capacity(record_count);
        for idx in 0..prefix_record_count {
            let start = idx * record_size;
            let slice = &buffer_slice[start..start + record_size];
            results.push(f(slice, idx));
        }

        // Aligned region
        let aligned_slice = &buffer_slice[aligned_record_offset..];
        let aligned_bytes = aligned_slice.len();
        let _aligned_record_count = aligned_bytes / record_size;

        // Calculate chunk size that is multiple of both record_size and cache line size
        let chunk_size = lcm(record_size, CACHE_LINE_SIZE);
        if chunk_size == 0 {
            // Should not happen since record_size > 0 and CACHE_LINE_SIZE > 0
            return Err(DbError::InvalidOffset {
                table: self.name.clone(),
                offset: 0,
                max: total_bytes.saturating_sub(record_size),
            });
        }

        // Process aligned region in parallel chunks
        let chunk_results: Vec<Vec<R>> = aligned_slice
            .par_chunks_exact(chunk_size)
            .enumerate()
            .map(|(chunk_idx, chunk)| {
                let start_record_idx = prefix_record_count + (chunk_idx * chunk_size) / record_size;
                let chunk_record_count = chunk_size / record_size;
                let mut chunk_results = Vec::with_capacity(chunk_record_count);
                for sub_idx in 0..chunk_record_count {
                    let record_idx = start_record_idx + sub_idx;
                    let start = sub_idx * record_size;
                    let slice = &chunk[start..start + record_size];
                    chunk_results.push(f(slice, record_idx));
                }
                chunk_results
            })
            .collect();

        // Flatten chunk results in order
        for chunk_vec in chunk_results {
            results.extend(chunk_vec);
        }

        // Process suffix records (after last full chunk) sequentially
        let processed_records =
            prefix_record_count + (aligned_bytes / chunk_size) * (chunk_size / record_size);
        for idx in processed_records..record_count {
            let start = idx * record_size;
            let slice = &buffer_slice[start..start + record_size];
            results.push(f(slice, idx));
        }

        Ok(results)
    }
}
