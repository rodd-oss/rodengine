//! Matchit routing configuration.

use std::sync::Arc;

use hyper::{body::Bytes, Request, Response};
use matchit::Router as MatchitRouter;
use tokio::sync::mpsc;

use crate::handlers;
use in_mem_db_core::{config::DbConfig, database::Database};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Database instance
    pub db: Arc<Database>,
    /// Database configuration
    pub config: Arc<DbConfig>,
    /// API request sender to runtime
    pub api_tx: mpsc::Sender<in_mem_db_runtime::ApiRequest>,
}

/// HTTP request router.
pub struct Router {
    inner: MatchitRouter<RouteHandler>,
    state: AppState,
}

impl Router {
    /// Creates a new router with default routes.
    pub fn new(
        db: Arc<Database>,
        config: Arc<DbConfig>,
        api_tx: mpsc::Sender<in_mem_db_runtime::ApiRequest>,
    ) -> Self {
        let mut router = MatchitRouter::new();

        // Table DDL endpoints
        router
            .insert("/tables", RouteHandler::Table)
            .expect("Failed to insert /tables route");
        router
            .insert("/tables/:name", RouteHandler::Table)
            .expect("Failed to insert /tables/:name route");
        router
            .insert("/tables/:name/fields", RouteHandler::Field)
            .expect("Failed to insert /tables/:name/fields route");
        router
            .insert("/tables/:name/fields/:field", RouteHandler::Field)
            .expect("Failed to insert /tables/:name/fields/:field route");

        // Record CRUD endpoints
        router
            .insert("/tables/:name/records", RouteHandler::Record)
            .expect("Failed to insert /tables/:name/records route");
        router
            .insert("/tables/:name/records/:id", RouteHandler::Record)
            .expect("Failed to insert /tables/:name/records/:id route");

        // Relation endpoints
        router
            .insert("/relations", RouteHandler::Relation)
            .expect("Failed to insert /relations route");
        router
            .insert("/relations/:id", RouteHandler::Relation)
            .expect("Failed to insert /relations/:id route");

        // RPC endpoint
        router
            .insert("/rpc/:name", RouteHandler::Rpc)
            .expect("Failed to insert /rpc/:name route");

        Self {
            inner: router,
            state: AppState { db, config, api_tx },
        }
    }

    /// Routes an incoming request to the appropriate handler.
    ///
    /// # Arguments
    /// * `req` - HTTP request
    ///
    /// # Returns
    /// `Result<Response<Bytes>, RouterError>` containing the response or an error.
    pub async fn route(
        &self,
        req: Request<hyper::body::Incoming>,
    ) -> Result<Response<Bytes>, RouterError> {
        let path = req.uri().path().to_string();
        let _method = req.method().as_str();

        // Match the route
        match self.inner.at(&path) {
            Ok(matched) => {
                let handler = matched.value;
                handler
                    .handle(req, matched.params, self.state.clone())
                    .await
            }
            Err(_) => {
                // Return 404 for unmatched routes
                let error_response = crate::handlers::error_response(
                    404,
                    "Not Found".to_string(),
                    Some(format!("No route found for {}", path)),
                );
                let body = serde_json::to_vec(&error_response).map_err(|e| {
                    RouterError::InternalError(format!("Failed to serialize error response: {}", e))
                })?;
                Ok(Response::builder()
                    .status(404)
                    .header("Content-Type", "application/json")
                    .body(Bytes::from(body))
                    .map_err(|e| {
                        RouterError::InternalError(format!("Failed to build response: {}", e))
                    })?)
            }
        }
    }
}

/// Route handler function.
enum RouteHandler {
    Table,
    Field,
    Record,
    Relation,
    Rpc,
}

impl RouteHandler {
    /// Handles a request with the given route parameters.
    async fn handle(
        &self,
        req: Request<hyper::body::Incoming>,
        params: matchit::Params<'_, '_>,
        state: AppState,
    ) -> Result<Response<Bytes>, RouterError> {
        match self {
            RouteHandler::Table => {
                let has_name_param = params.get("name").is_some();
                if req.method() == hyper::Method::POST && has_name_param {
                    handlers::create_table(req, params, state).await
                } else if req.method() == hyper::Method::DELETE && has_name_param {
                    handlers::delete_table(req, params, state).await
                } else if req.method() == hyper::Method::GET && !has_name_param {
                    handlers::list_tables(req, params, state).await
                } else {
                    Err(RouterError::MethodNotAllowed)
                }
            }
            RouteHandler::Field => {
                let has_field_param = params.get("field").is_some();
                if req.method() == hyper::Method::POST && !has_field_param {
                    handlers::add_field(req, params, state).await
                } else if req.method() == hyper::Method::DELETE && has_field_param {
                    handlers::remove_field(req, params, state).await
                } else {
                    Err(RouterError::MethodNotAllowed)
                }
            }
            RouteHandler::Record => {
                let has_id_param = params.get("id").is_some();
                if req.method() == hyper::Method::POST && !has_id_param {
                    handlers::create_record(req, params, state).await
                } else if req.method() == hyper::Method::GET && has_id_param {
                    handlers::read_record(req, params, state).await
                } else if req.method() == hyper::Method::GET && !has_id_param {
                    handlers::query_records(req, params, state).await
                } else if req.method() == hyper::Method::PUT && has_id_param {
                    handlers::update_record(req, params, state).await
                } else if req.method() == hyper::Method::PATCH && has_id_param {
                    handlers::partial_update_record(req, params, state).await
                } else if req.method() == hyper::Method::DELETE && has_id_param {
                    handlers::delete_record(req, params, state).await
                } else {
                    Err(RouterError::MethodNotAllowed)
                }
            }
            RouteHandler::Relation => {
                let has_id_param = params.get("id").is_some();
                if req.method() == hyper::Method::POST && !has_id_param {
                    handlers::create_relation(req, params, state).await
                } else if req.method() == hyper::Method::DELETE && has_id_param {
                    handlers::delete_relation(req, params, state).await
                } else {
                    Err(RouterError::MethodNotAllowed)
                }
            }
            RouteHandler::Rpc => {
                if req.method() == hyper::Method::POST {
                    handlers::rpc(req, params, state).await
                } else {
                    Err(RouterError::MethodNotAllowed)
                }
            }
        }
    }
}

/// Router error type.
#[derive(Debug)]
pub enum RouterError {
    MethodNotAllowed,
    InternalError(String),
    Timeout,
    BadRequest(String),
    NotFound(String),
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterError::MethodNotAllowed => write!(f, "Method Not Allowed"),
            RouterError::InternalError(msg) => write!(f, "Internal Error: {}", msg),
            RouterError::Timeout => write!(f, "Request Timeout"),
            RouterError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            RouterError::NotFound(msg) => write!(f, "Not Found: {}", msg),
        }
    }
}

impl std::error::Error for RouterError {}

impl From<RouterError> for Response<Bytes> {
    fn from(err: RouterError) -> Self {
        let (status, message) = match &err {
            RouterError::MethodNotAllowed => (405, "Method Not Allowed"),
            RouterError::InternalError(msg) => (500, msg.as_str()),
            RouterError::Timeout => (408, "Request Timeout"),
            RouterError::BadRequest(msg) => (400, msg.as_str()),
            RouterError::NotFound(msg) => (404, msg.as_str()),
        };

        let error_response = crate::handlers::error_response(status, message.to_string(), None);
        // Note: We use expect here because if we can't serialize an error response,
        // we're in a truly unrecoverable state. This is a fallback for when error
        // handling itself fails.
        let body = serde_json::to_vec(&error_response)
            .unwrap_or_else(|e| format!("{{\"success\":false,\"error\":{{\"code\":\"500\",\"message\":\"Failed to serialize error: {}\",\"details\":null}}}}", e).into_bytes());

        Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Bytes::from(body))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(500)
                    .body(Bytes::from("Internal Server Error"))
                    .expect("Failed to build fallback error response")
            })
    }
}
