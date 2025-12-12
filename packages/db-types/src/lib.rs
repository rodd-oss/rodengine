//! Shared types and utilities for the database system.
//!
//! This crate defines field types, table schemas, and other shared data structures.

pub mod field;
pub mod table;
pub mod types;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}