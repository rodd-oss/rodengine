//! Tick-based runtime and procedure execution system

mod api_handlers;
mod api_request;
mod runtime;
mod tick_phases;

pub use api_handlers::ApiHandlers;
pub use api_request::*;
pub use runtime::Runtime;
pub use tick_phases::TickPhaseProcessor;

mod procedure;
pub use procedure::{ProcedureFn, ProcedureRegistry};

/// Result type for runtime operations
pub type Result<T> = std::result::Result<T, in_mem_db_core::error::DbError>;

/// Response sender for API requests
pub type ResponseSender = tokio::sync::oneshot::Sender<Result<serde_json::Value>>;
