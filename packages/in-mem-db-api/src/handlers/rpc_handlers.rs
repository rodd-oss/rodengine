//! RPC (Remote Procedure Call) operation handlers.

use hyper::{body::Bytes, Request, Response};

use crate::router::{AppState, RouterError};
use in_mem_db_runtime::ApiRequest;

use super::request_utils::{
    build_response, map_db_error_to_router_error, read_request_body_with_timeout,
    wait_for_response_with_timeout,
};
use super::response::success_response;

/// Executes a registered procedure via RPC.
///
/// # Endpoint
/// `POST /rpc/{name}`
///
/// # Request Body
/// ```json
/// {
///   "table": "users",
///   "filter_field": "active",
///   "filter_value": false,
///   "set_field": "active",
///   "set_value": true
/// }
/// ```
///
/// # Response
/// - **200 OK**: Returns procedure result
/// ```json
/// {
///   "affected": 42
/// }
/// ```
///
/// # Errors
/// - **400 Bad Request**: Procedure not found or invalid parameters
/// - **404 Not Found**: Procedure not registered
/// - **500 Internal Server Error**: Procedure execution error or runtime error
///
/// # Notes
/// - Procedures run in isolated transactions
/// - Can perform parallel operations across CPU cores
/// - Changes are atomic (all-or-nothing)
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:8080/rpc/bulk_update \
///   -H "Content-Type: application/json" \
///   -d '{"table": "users", "filter_field": "active", "filter_value": false, "set_field": "active", "set_value": true}'
/// ```
pub async fn rpc(
    req: Request<hyper::body::Incoming>,
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let procedure_name = params.get("name").unwrap_or("unknown").to_string();

    // Read JSON params from request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let params_json: serde_json::Value = if body_bytes.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&body_bytes)
            .map_err(|e| RouterError::BadRequest(format!("Failed to parse JSON params: {}", e)))?
    };

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::Rpc {
        name: procedure_name.clone(),
        params: params_json,
        response: tx,
    };
    state
        .api_tx
        .send(api_request)
        .await
        .map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    let response_json = result.map_err(map_db_error_to_router_error)?;
    let api_response = success_response(response_json);

    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(200, json)
}
