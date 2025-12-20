//! CRUD (Create, Read, Update, Delete) operation handlers.

use hyper::{body::Bytes, Request, Response};

use crate::router::{AppState, RouterError};
use in_mem_db_runtime::{ApiRequest, CrudOperation};

use super::request_utils::{
    build_empty_response, build_response, map_db_error_to_router_error,
    read_request_body_with_timeout, wait_for_response_with_timeout, CreateRecordRequest,
    CreateRecordResponse, PartialUpdateRequest, UpdateRecordRequest,
};
use super::response::success_response;

/// Creates a new record in a table.
///
/// # Endpoint
/// `POST /tables/{name}/records`
///
/// # Request Body
/// ```json
/// {
///   "values": [123, "entity_name", [1.0, 2.0, 3.0], true]
/// }
/// ```
///
/// # Response
/// - **201 Created**: Returns assigned record ID
/// ```json
/// {
///   "id": 123
/// }
/// ```
///
/// # Errors
/// - **400 Bad Request**: Table not found, wrong number of values, or type mismatch
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - Values must be provided in the same order as table fields
/// - Record IDs are auto-incremented
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:8080/tables/users/records \
///   -H "Content-Type: application/json" \
///   -d '{"values": [1, "John Doe", true]}'
/// ```
pub async fn create_record(
    req: Request<hyper::body::Incoming>,
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateRecordRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Create {
            values: request.values,
        },
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

    // Expect response contains "id": u64
    let id = response_json
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let response_data = CreateRecordResponse { id };
    let api_response = success_response(response_data);

    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(201, json)
}

/// Reads a record from a table.
///
/// # Endpoint
/// `GET /tables/{name}/records/{id}`
///
/// # Response
/// - **200 OK**: Returns record data as JSON
/// ```json
/// {
///   "id": 123,
///   "name": "entity_name",
///   "position": [1.0, 2.0, 3.0],
///   "active": true
/// }
/// ```
///
/// # Errors
/// - **400 Bad Request**: Table not found or invalid record ID
/// - **404 Not Found**: Record not found
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Example
/// ```bash
/// curl http://localhost:8080/tables/users/records/1
/// ```
pub async fn read_record(
    _req: Request<hyper::body::Incoming>,
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::BadRequest(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Read { id: record_id },
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

    // Return as JSON with consistent format
    let api_response = success_response(response_json);
    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(200, json)
}

/// Fully updates a record.
///
/// # Endpoint
/// `PUT /tables/{name}/records/{id}`
///
/// # Request Body
/// ```json
/// {
///   "values": [123, "updated_name", [4.0, 5.0, 6.0], false]
/// }
/// ```
///
/// # Response
/// - **204 No Content**: Record successfully updated
///
/// # Errors
/// - **400 Bad Request**: Table not found, invalid record ID, wrong number of values, or type mismatch
/// - **404 Not Found**: Record not found
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - This replaces the entire record with new values
/// - Values must be provided in the same order as table fields
///
/// # Example
/// ```bash
/// curl -X PUT http://localhost:8080/tables/users/records/1 \
///   -H "Content-Type: application/json" \
///   -d '{"values": [1, "Jane Doe", false]}'
/// ```
pub async fn update_record(
    req: Request<hyper::body::Incoming>,
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::BadRequest(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: UpdateRecordRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Update {
            id: record_id,
            values: request.values,
        },
        response: tx,
    };
    state
        .api_tx
        .send(api_request)
        .await
        .map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    build_empty_response(204)
}

/// Partially updates a record.
///
/// # Endpoint
/// `PATCH /tables/{name}/records/{id}`
///
/// # Request Body
/// ```json
/// {
///   "updates": {
///     "name": "updated_name",
///     "active": false
///   }
/// }
/// ```
///
/// # Response
/// - **204 No Content**: Record successfully updated
///
/// # Errors
/// - **400 Bad Request**: Table not found, invalid record ID, field not found, or type mismatch
/// - **404 Not Found**: Record not found
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - Only specified fields are updated
/// - Fields not in the updates object remain unchanged
///
/// # Example
/// ```bash
/// curl -X PATCH http://localhost:8080/tables/users/records/1 \
///   -H "Content-Type: application/json" \
///   -d '{"updates": {"name": "Updated Name", "active": false}}'
/// ```
pub async fn partial_update_record(
    req: Request<hyper::body::Incoming>,
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::BadRequest(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: PartialUpdateRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // For partial update, we need to read the current record first
    // then apply the updates and send as a full update
    // This is a simplified implementation - in production we'd want
    // a more efficient partial update mechanism

    // First, read current record
    let (read_tx, read_rx) = tokio::sync::oneshot::channel();
    let read_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Read { id: record_id },
        response: read_tx,
    };
    state
        .api_tx
        .send(read_request)
        .await
        .map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    let current_record = read_rx
        .await
        .map_err(|e| RouterError::InternalError(format!("Response channel closed: {}", e)))?
        .map_err(|e| RouterError::InternalError(format!("Runtime error reading record: {}", e)))?;

    // Apply updates to current record
    let mut updated_values = Vec::new();
    if let Some(current_obj) = current_record.as_object() {
        // Get table schema to know field order
        let table_ref = state
            .db
            .get_table(&table_name)
            .map_err(map_db_error_to_router_error)?;

        for field in &table_ref.fields {
            if let Some(update_value) = request.updates.get(&field.name) {
                updated_values.push(update_value.clone());
            } else if let Some(current_value) = current_obj.get(&field.name) {
                updated_values.push(current_value.clone());
            } else {
                return Err(RouterError::BadRequest(format!(
                    "Field {} not found in current record",
                    field.name
                )));
            }
        }
    } else {
        return Err(RouterError::BadRequest("Invalid record format".to_string()));
    }

    // Send update request
    let (update_tx, update_rx) = tokio::sync::oneshot::channel();
    let update_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Update {
            id: record_id,
            values: updated_values,
        },
        response: update_tx,
    };
    state
        .api_tx
        .send(update_request)
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send update request: {}", e)))?;

    // Wait for response
    let result = update_rx
        .await
        .map_err(|e| RouterError::InternalError(format!("Response channel closed: {}", e)))?;

    result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    build_empty_response(204)
}

/// Deletes a record.
///
/// # Endpoint
/// `DELETE /tables/{name}/records/{id}`
///
/// # Response
/// - **204 No Content**: Record successfully deleted
///
/// # Errors
/// - **400 Bad Request**: Table not found or invalid record ID
/// - **404 Not Found**: Record not found
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - This is a soft delete (record is marked as deleted)
/// - Deleted records can be compacted later
///
/// # Example
/// ```bash
/// curl -X DELETE http://localhost:8080/tables/users/records/1
/// ```
pub async fn delete_record(
    _req: Request<hyper::body::Incoming>,
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::BadRequest(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Delete { id: record_id },
        response: tx,
    };
    state
        .api_tx
        .send(api_request)
        .await
        .map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    build_empty_response(204)
}
