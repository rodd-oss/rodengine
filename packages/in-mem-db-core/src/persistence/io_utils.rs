//! I/O utilities for persistence operations.

use std::io::ErrorKind;

use crate::error::DbError;

/// Classifies I/O errors into specific DbError variants.
pub fn classify_io_error(error: std::io::Error, context: &str) -> DbError {
    match error.kind() {
        ErrorKind::StorageFull | ErrorKind::OutOfMemory => {
            DbError::DiskFull(format!("{}: {}", context, error))
        }
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted => {
            DbError::TransientIoError(format!("{}: {}", context, error))
        }
        ErrorKind::NotFound
        | ErrorKind::PermissionDenied
        | ErrorKind::AlreadyExists
        | ErrorKind::InvalidInput
        | ErrorKind::InvalidData => DbError::IoError(format!("{}: {}", context, error)),
        _ => DbError::IoError(format!("{}: {}", context, error)),
    }
}

/// Retries an operation that may fail with transient I/O errors.
pub fn retry_io_operation<F, T>(
    operation: F,
    max_retries: u32,
    retry_delay_ms: u64,
    context: &str,
) -> Result<T, DbError>
where
    F: Fn() -> Result<T, DbError>,
{
    let mut attempt = 0;
    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(err) => {
                attempt += 1;
                if attempt > max_retries {
                    return Err(err);
                }

                // Only retry transient I/O errors
                if let DbError::TransientIoError(_) = err {
                    tracing::warn!(
                        "Transient I/O error in {} (attempt {}/{}): {}",
                        context,
                        attempt,
                        max_retries,
                        err
                    );

                    // Sleep before retry
                    if retry_delay_ms > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(retry_delay_ms));
                    }

                    continue;
                }

                // Non-transient error, return immediately
                return Err(err);
            }
        }
    }
}
