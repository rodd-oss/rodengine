//! Core storage engine for in-memory relational database.
//!
//! Provides atomic buffer management, type system, table schema,
//! transaction isolation, and persistence.

pub mod atomic_buffer;
pub mod config;
pub mod database;
pub mod error;
pub mod persistence;
pub mod table;
pub mod transaction;
pub mod types;

pub use database::ProcedureFn;
