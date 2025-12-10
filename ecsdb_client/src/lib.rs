//! Client library for ECS Database replication.
//!
//! Provides a lightweight in‑memory database that synchronizes with a remote
//! ECSDb server via delta‑based replication.

pub mod client_db;
pub mod error;
pub mod sync;

pub use client_db::ClientDB;
pub use error::{ClientError, Result};

/// Re‑exports from ecsdb for convenience.
pub use ecsdb::{Component, ZeroCopyComponent};
