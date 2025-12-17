//! Tick-based runtime and procedure execution system.
//!
//! Provides runtime loop with configurable tickrate, procedure scheduling,
//! parallel execution, and rate limiting.

pub mod metrics;
pub mod procedure;
pub mod runtime;
pub mod scheduler;
