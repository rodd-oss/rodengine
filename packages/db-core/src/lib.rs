//! Core storage engine for in-memory relational database.
//!
//! This crate provides the foundational storage layer with zero-copy access,
//! cache-efficient layout, and atomic operations.

pub mod schema;
pub mod storage;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
