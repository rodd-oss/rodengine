//! Integration test suite following the test plan.
//!
//! Tests are organized by section from the test plan:
//! 1. Basic CRUD
//! 6. Persistence integration
//! 8. Full system integration

pub mod basic_crud;
pub mod end_to_end_tests;
pub mod helpers;
pub mod persistence_tests;
pub mod system_smoke_tests;
