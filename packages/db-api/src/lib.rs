//! REST and RPC API layer for the database.
//!
//! This crate provides HTTP endpoints for schema management, CRUD operations,
//! and custom procedures using axum and tokio.

pub mod rest;
pub mod rpc;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
