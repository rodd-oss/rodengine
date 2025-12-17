//! REST API server for in-memory database.
//!
//! Provides HTTP endpoints for CRUD operations, DDL commands,
//! RPC procedures, and request routing.

pub mod handlers;
pub mod middleware;
pub mod router;
pub mod server;
