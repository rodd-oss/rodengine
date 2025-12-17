//! Integration test suite following the test plan.
//!
//! Tests are organized by section from the integration test plan:
//! 1. Basic CRUD
//! 2. Transaction integration
//! 3. Concurrency integration
//! 4. Procedure integration
//! 5. Runtime loop integration
//! 6. Persistence integration
//! 7. Relation integration
//! 8. Full system integration
//! 9. Failure mode integration
//! 10. Performance regression integration

mod basic_crud;
mod helpers;