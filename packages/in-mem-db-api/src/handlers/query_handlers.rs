//! Query and list operation handlers.

use hyper::{body::Bytes, Request, Response};

use crate::router::{AppState, RouterError};
use in_mem_db_runtime::ApiRequest;

use super::request_utils::{
    build_response, map_db_error_to_router_error, parse_query_params,
    wait_for_response_with_timeout,
};
use super::response::success_response;

/// Lists all tables in the database.
///
/// # Endpoint
/// `GET /tables`
///
/// # Response
/// - **200 OK**: Returns list of tables
/// ```json
/// [
///   "users",
///   "posts",
///   "comments"
/// ]
/// ```
///
/// # Errors
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Example
/// ```bash
/// curl http://localhost:8080/tables
/// ```
pub async fn list_tables(
    _req: Request<hyper::body::Incoming>,
    _params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::ListTables { response: tx };
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

/// Query records with filtering and pagination.
///
/// # Endpoint
/// `GET /tables/{name}/records`
///
/// # Query Parameters
/// - `limit`: Maximum number of records to return
/// - `offset`: Number of records to skip
/// - `{field}`: Filter by field value (e.g., `active=true`)
///
/// # Response
/// - **200 OK**: Returns matching records
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "User1",
///     "active": true
///   },
///   {
///     "id": 2,
///     "name": "User2",
///     "active": false
///   }
/// ]
/// ```
///
/// # Errors
/// - **400 Bad Request**: Table not found or invalid query parameters
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - Filters perform exact matches on field values
/// - Results are returned in insertion order
///
/// # Examples
/// ```bash
/// # Get first 10 records
/// curl "http://localhost:8080/tables/users/records?limit=10"
///
/// # Get records 11-20
/// curl "http://localhost:8080/tables/users/records?limit=10&offset=10"
///
/// # Get active users
/// curl "http://localhost:8080/tables/users/records?active=true"
///
/// # Get specific user by name
/// curl "http://localhost:8080/tables/users/records?name=John%20Doe"
/// ```
pub async fn query_records(
    req: Request<hyper::body::Incoming>,
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Parse query parameters from URL
    let query_params = if let Some(query_str) = req.uri().query() {
        parse_query_params(Some(query_str))?
    } else {
        in_mem_db_runtime::QueryParams {
            limit: None,
            offset: None,
            filters: std::collections::HashMap::new(),
        }
    };

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::QueryRecords {
        table: table_name.clone(),
        query: query_params,
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
