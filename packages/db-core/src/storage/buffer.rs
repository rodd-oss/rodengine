//! TableBuffer - A Vec<u8> storage buffer for table data with zero-copy access.
//!
//! This buffer provides contiguous memory storage with capacity management,
//! designed for cache-efficient access and future unsafe casting to field types.
