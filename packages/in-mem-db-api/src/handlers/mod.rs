//! HTTP endpoint implementations for CRUD, DDL, and RPC.

use http_body_util::BodyExt;
use hyper::{body::Bytes, Request, Response};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::time;

use crate::router::{AppState, RouterError};
use in_mem_db_core::table::Field;
use in_mem_db_runtime::{ApiRequest, CrudOperation, QueryParams};

/// Type alias for matchit parameters with explicit lifetimes
type MatchitParams<'a, 'b> = matchit::Params<'a, 'b>;

/// Consistent API response wrapper for success responses
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// Always true for success responses
    pub success: bool,
    /// Response data
    pub data: T,
}

/// Consistent API error response wrapper
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// Error code (HTTP status code as string)
    pub code: String,
    /// Error message
    pub message: String,
    /// Optional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Consistent error response wrapper
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Always false for error responses
    pub success: bool,
    /// Error information
    pub error: ApiError,
}

/// Helper to create success response
pub fn success_response<T: Serialize>(data: T) -> ApiResponse<T> {
    ApiResponse {
        success: true,
        data,
    }
}

/// Helper to create error response
pub fn error_response(code: u16, message: String, details: Option<String>) -> ErrorResponse {
    ErrorResponse {
        success: false,
        error: ApiError {
            code: code.to_string(),
            message,
            details,
        },
    }
}

/// Helper to build HTTP response with proper error handling
fn build_response(status: u16, json: Vec<u8>) -> Result<Response<Bytes>, RouterError> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .map_err(|e| RouterError::InternalError(format!("Failed to build response: {}", e)))
}

/// Helper to build empty HTTP response (for 204 No Content)
fn build_empty_response(status: u16) -> Result<Response<Bytes>, RouterError> {
    Response::builder()
        .status(status)
        .body(Bytes::new())
        .map_err(|e| RouterError::InternalError(format!("Failed to build response: {}", e)))
}

/// Helper function to read request body with timeout
async fn read_request_body_with_timeout(
    req: Request<hyper::body::Incoming>,
    timeout_ms: u64,
) -> Result<Bytes, RouterError> {
    let timeout_duration = time::Duration::from_millis(timeout_ms);
    let body = time::timeout(timeout_duration, req.collect())
        .await
        .map_err(|_| RouterError::Timeout)?
        .map_err(|e| RouterError::InternalError(format!("Failed to read request body: {}", e)))?;
    Ok(body.to_bytes())
}

/// Helper function to wait for response with timeout
async fn wait_for_response_with_timeout<T>(
    rx: oneshot::Receiver<T>,
    timeout_ms: u64,
) -> Result<T, RouterError> {
    let timeout_duration = time::Duration::from_millis(timeout_ms);
    time::timeout(timeout_duration, rx)
        .await
        .map_err(|_| RouterError::Timeout)?
        .map_err(|e| RouterError::InternalError(format!("Response channel closed: {}", e)))
}

/// Map DbError to appropriate RouterError
fn map_db_error_to_router_error(e: in_mem_db_core::error::DbError) -> RouterError {
    match e {
        in_mem_db_core::error::DbError::TableNotFound { .. }
        | in_mem_db_core::error::DbError::FieldNotFound { .. }
        | in_mem_db_core::error::DbError::RecordNotFound { .. }
        | in_mem_db_core::error::DbError::ProcedureNotFound { .. } => {
            RouterError::NotFound(e.to_string())
        }
        in_mem_db_core::error::DbError::FieldAlreadyExists { .. }
        | in_mem_db_core::error::DbError::FieldExceedsRecordSize { .. }
        | in_mem_db_core::error::DbError::CapacityOverflow { .. }
        | in_mem_db_core::error::DbError::TypeMismatch { .. }
        | in_mem_db_core::error::DbError::InvalidOffset { .. }
        | in_mem_db_core::error::DbError::TableAlreadyExists(_) => {
            RouterError::BadRequest(e.to_string())
        }
        _ => RouterError::InternalError(format!("Runtime error: {}", e)),
    }
}

/// Field definition for table creation.
#[derive(Debug, Deserialize, Serialize)]
pub struct FieldDefinition {
    /// Field name
    pub name: String,
    /// Type identifier (e.g., "u64", "string", "3xf32")
    pub r#type: String,
}

/// Request to create a table.
#[derive(Debug, Deserialize)]
pub struct CreateTableRequest {
    /// Field definitions
    pub fields: Vec<FieldDefinition>,
}

/// Response from creating a table.
#[derive(Debug, Serialize)]
pub struct CreateTableResponse {
    /// Table name
    pub table: String,
    /// Record size in bytes
    pub record_size: usize,
}

/// Request to add a field to a table.
#[derive(Debug, Deserialize)]
pub struct AddFieldRequest {
    /// Field name
    pub name: String,
    /// Type identifier
    pub r#type: String,
}

/// Response from adding a field.
#[derive(Debug, Serialize)]
pub struct AddFieldResponse {
    /// Field offset within record
    pub offset: usize,
    /// New record size
    pub record_size: usize,
}

/// Request to create a record.
#[derive(Debug, Deserialize)]
pub struct CreateRecordRequest {
    /// Field values in the same order as table fields
    pub values: Vec<serde_json::Value>,
}

/// Response from creating a record.
#[derive(Debug, Serialize)]
pub struct CreateRecordResponse {
    /// Assigned record ID
    pub id: u64,
}

/// Request to update a record.
#[derive(Debug, Deserialize)]
pub struct UpdateRecordRequest {
    /// New field values in the same order as table fields
    pub values: Vec<serde_json::Value>,
}

/// Request for partial record update.
#[derive(Debug, Deserialize)]
pub struct PartialUpdateRequest {
    /// Field name to value mapping
    pub updates: std::collections::HashMap<String, serde_json::Value>,
}

/// Request to create a relation.
#[derive(Debug, Deserialize)]
pub struct CreateRelationRequest {
    /// Source table name
    pub from_table: String,
    /// Source field name
    pub from_field: String,
    /// Target table name
    pub to_table: String,
    /// Target field name
    pub to_field: String,
}

/// Response from creating a relation.
#[derive(Debug, Serialize)]
pub struct CreateRelationResponse {
    /// Relation ID
    pub id: String,
}

/// Creates a new table.
///
/// # Endpoint
/// `POST /tables/{name}`
///
/// # Request Body
/// ```json
/// {
///   "fields": [
///     {"name": "field1", "type": "u64"},
///     {"name": "field2", "type": "string"},
///     {"name": "field3", "type": "3xf32"}
///   ]
/// }
/// ```
///
/// # Response
/// - **201 Created**: Returns table name and record size
/// ```json
/// {
///   "table": "table_name",
///   "record_size": 24
/// }
/// ```
///
/// # Errors
/// - **400 Bad Request**: Invalid field definitions, unknown type, or table already exists
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:8080/tables/users \
///   -H "Content-Type: application/json" \
///   -d '{"fields": [{"name": "id", "type": "u64"}, {"name": "name", "type": "string"}]}'
/// ```
pub async fn create_table(
    req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Read and parse request body with timeout
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateTableRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // Create fields
    let type_registry = state.db.type_registry();
    let mut fields = Vec::new();

    for field_def in request.fields {
        let layout = type_registry.get(&field_def.r#type).ok_or_else(|| {
            RouterError::BadRequest(format!("Unknown type: {}", field_def.r#type))
        })?;

        // Calculate offset (simplified - actual offset calculation happens in Table::create)
        let offset = 0; // Will be recalculated by Table::create
        let field = Field::new(field_def.name, field_def.r#type, layout.clone(), offset);
        fields.push(field);
    }

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::CreateTable {
        name: table_name.clone(),
        fields,
        response: tx,
    };
    // Send via blocking task since api_tx is std::sync::mpsc
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    let response_json = result.map_err(map_db_error_to_router_error)?;

    // Convert to CreateTableResponse
    let record_size = response_json
        .get("record_size")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let response_data = CreateTableResponse {
        table: table_name,
        record_size: record_size as usize,
    };
    let api_response = success_response(response_data);

    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(201, json)
}

/// Deletes a table.
///
/// # Endpoint
/// `DELETE /tables/{name}`
///
/// # Response
/// - **204 No Content**: Table successfully deleted
///
/// # Errors
/// - **400 Bad Request**: Table not found
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Example
/// ```bash
/// curl -X DELETE http://localhost:8080/tables/users
/// ```
pub async fn delete_table(
    _req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::DeleteTable {
        name: table_name.clone(),
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    build_empty_response(204)
}

/// Adds a field to an existing table.
///
/// # Endpoint
/// `POST /tables/{name}/fields`
///
/// # Request Body
/// ```json
/// {
///   "name": "field_name",
///   "type": "field_type"
/// }
/// ```
///
/// # Response
/// - **200 OK**: Returns field offset and new record size
/// ```json
/// {
///   "offset": 24,
///   "record_size": 25
/// }
/// ```
///
/// # Errors
/// - **400 Bad Request**: Table not found, field already exists, or unknown type
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - This is a blocking operation that rewrites all existing records
/// - Existing records will have the new field initialized with default values
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:8080/tables/users/fields \
///   -H "Content-Type: application/json" \
///   -d '{"name": "email", "type": "string"}'
/// ```
pub async fn add_field(
    req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: AddFieldRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // Get type layout
    let type_registry = state.db.type_registry();
    let layout = type_registry
        .get(&request.r#type)
        .ok_or_else(|| RouterError::BadRequest(format!("Unknown type: {}", request.r#type)))?;

    // Create Field object (offset will be recalculated by runtime)
    let field = Field::new(
        request.name.clone(),
        request.r#type.clone(),
        layout.clone(),
        0,
    );

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::AddField {
        table: table_name.clone(),
        field,
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    let response_json = result.map_err(map_db_error_to_router_error)?;

    let offset = response_json
        .get("offset")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let record_size = response_json
        .get("record_size")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let response_data = AddFieldResponse {
        offset: offset as usize,
        record_size: record_size as usize,
    };
    let api_response = success_response(response_data);

    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(200, json)
}

/// Removes a field from a table.
///
/// # Endpoint
/// `DELETE /tables/{name}/fields/{field}`
///
/// # Response
/// - **204 No Content**: Field successfully removed
///
/// # Errors
/// - **400 Bad Request**: Table not found or field not found
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - This is a blocking operation that rewrites all existing records
/// - Data in the removed field is permanently lost
///
/// # Example
/// ```bash
/// curl -X DELETE http://localhost:8080/tables/users/fields/email
/// ```
pub async fn remove_field(
    _req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let field_name = params.get("field").unwrap_or("unknown").to_string();

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::RemoveField {
        table: table_name.clone(),
        field_name: field_name.clone(),
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    build_empty_response(204)
}

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
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateRecordRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Create {
            values: request.values,
        },
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

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
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::BadRequest(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Read { id: record_id },
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

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
    params: MatchitParams<'_, '_>,
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
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Update {
            id: record_id,
            values: request.values,
        },
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

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
    params: MatchitParams<'_, '_>,
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
    let (read_tx, read_rx) = oneshot::channel();
    let read_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Read { id: record_id },
        response: read_tx,
    };
    let api_tx_clone = state.api_tx.clone();
    let send_result = tokio::task::spawn_blocking(move || api_tx_clone.send(read_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send read request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

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
    let (update_tx, update_rx) = oneshot::channel();
    let update_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Update {
            id: record_id,
            values: updated_values,
        },
        response: update_tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(update_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send update request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

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
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::BadRequest(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::Crud {
        table: table_name.clone(),
        operation: CrudOperation::Delete { id: record_id },
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    build_empty_response(204)
}

/// Creates a relation between tables.
///
/// # Endpoint
/// `POST /relations`
///
/// # Request Body
/// ```json
/// {
///   "from_table": "users",
///   "from_field": "id",
///   "to_table": "posts",
///   "to_field": "user_id"
/// }
/// ```
///
/// # Response
/// - **201 Created**: Returns relation ID
/// ```json
/// {
///   "id": "users.id->posts.user_id"
/// }
/// ```
///
/// # Errors
/// - **400 Bad Request**: Table not found, field not found, or invalid relation
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - Creates a foreign key relationship between tables
/// - Relations are used for referential integrity and joins
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:8080/relations \
///   -H "Content-Type: application/json" \
///   -d '{"from_table": "users", "from_field": "id", "to_table": "posts", "to_field": "user_id"}'
/// ```
pub async fn create_relation(
    req: Request<hyper::body::Incoming>,
    _params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateRelationRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::CreateRelation {
        from_table: request.from_table.clone(),
        from_field: request.from_field.clone(),
        to_table: request.to_table.clone(),
        to_field: request.to_field.clone(),
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    let response_json = result.map_err(map_db_error_to_router_error)?;

    // Extract relation ID from response
    let id = response_json
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?
        .to_string();

    let response_data = CreateRelationResponse { id };
    let api_response = success_response(response_data);

    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(201, json)
}

/// Deletes a relation.
///
/// # Endpoint
/// `DELETE /relations/{id}`
///
/// # Response
/// - **204 No Content**: Relation successfully deleted
///
/// # Errors
/// - **400 Bad Request**: Relation not found
/// - **500 Internal Server Error**: Runtime error or channel communication failure
///
/// # Notes
/// - Removes the foreign key relationship
/// - Does not affect existing data
///
/// # Example
/// ```bash
/// curl -X DELETE http://localhost:8080/relations/users.id->posts.user_id
/// ```
pub async fn delete_relation(
    _req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let relation_id = params.get("id").unwrap_or("unknown").to_string();

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::DeleteRelation {
        id: relation_id.clone(),
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    build_empty_response(204)
}

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
    params: MatchitParams<'_, '_>,
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
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::Rpc {
        name: procedure_name.clone(),
        params: params_json,
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    let response_json = result.map_err(map_db_error_to_router_error)?;
    let api_response = success_response(response_json);

    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(200, json)
}

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
    _params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::ListTables { response: tx };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

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
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Parse query parameters from URL
    let query_params = if let Some(query_str) = req.uri().query() {
        parse_query_params(Some(query_str))?
    } else {
        QueryParams {
            limit: None,
            offset: None,
            filters: std::collections::HashMap::new(),
        }
    };

    // Send request to runtime
    let (tx, rx) = oneshot::channel();
    let api_request = ApiRequest::QueryRecords {
        table: table_name.clone(),
        query: query_params,
        response: tx,
    };
    let send_result = tokio::task::spawn_blocking(move || state.api_tx.send(api_request))
        .await
        .map_err(|e| RouterError::InternalError(format!("Failed to send request: {}", e)))?;
    send_result.map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

    // Wait for response
    let result = wait_for_response_with_timeout(rx, state.config.response_timeout_ms).await?;

    let response_json = result.map_err(map_db_error_to_router_error)?;
    let api_response = success_response(response_json);

    let json = serde_json::to_vec(&api_response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    build_response(200, json)
}

/// Parse query parameters from URL query string.
fn parse_query_params(query_str: Option<&str>) -> Result<QueryParams, RouterError> {
    let mut limit = None;
    let mut offset = None;
    let mut filters = std::collections::HashMap::new();

    if let Some(query_str) = query_str {
        for pair in query_str.split('&') {
            let parts: Vec<&str> = pair.split('=').collect();
            if parts.len() != 2 {
                continue;
            }
            let key = parts[0];
            let encoded_value = parts[1];
            let decoded_value = percent_decode_str(encoded_value).decode_utf8_lossy();

            match key {
                "limit" => {
                    limit = Some(decoded_value.parse().map_err(|e| {
                        RouterError::BadRequest(format!(
                            "Invalid limit value '{}': {}",
                            decoded_value, e
                        ))
                    })?);
                }
                "offset" => {
                    offset = Some(decoded_value.parse().map_err(|e| {
                        RouterError::BadRequest(format!(
                            "Invalid offset value '{}': {}",
                            decoded_value, e
                        ))
                    })?);
                }
                _ => {
                    // Try to parse as JSON, fall back to string
                    let json_value = serde_json::from_str(&decoded_value)
                        .unwrap_or_else(|_| serde_json::Value::String(decoded_value.to_string()));
                    filters.insert(key.to_string(), json_value);
                }
            }
        }
    }

    Ok(QueryParams {
        limit,
        offset,
        filters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_params() {
        // Test empty query
        let params = parse_query_params(None).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
        assert!(params.filters.is_empty());

        // Test limit and offset
        let params = parse_query_params(Some("limit=10&offset=5")).unwrap();
        assert_eq!(params.limit, Some(10));
        assert_eq!(params.offset, Some(5));
        assert!(params.filters.is_empty());

        // Test field filters
        let params = parse_query_params(Some("name=User1&active=true")).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
        assert_eq!(params.filters.len(), 2);
        assert_eq!(
            params.filters.get("name").unwrap(),
            &serde_json::json!("User1")
        );
        assert_eq!(
            params.filters.get("active").unwrap(),
            &serde_json::json!(true)
        );

        // Test mixed parameters
        let params = parse_query_params(Some("limit=5&offset=2&active=false&id=42")).unwrap();
        assert_eq!(params.limit, Some(5));
        assert_eq!(params.offset, Some(2));
        assert_eq!(params.filters.len(), 2);
        assert_eq!(
            params.filters.get("active").unwrap(),
            &serde_json::json!(false)
        );
        assert_eq!(params.filters.get("id").unwrap(), &serde_json::json!(42));

        // Test invalid limit
        let result = parse_query_params(Some("limit=abc"));
        assert!(result.is_err());

        // Test invalid offset
        let result = parse_query_params(Some("offset=xyz"));
        assert!(result.is_err());

        // Test invalid JSON in filter (should be treated as string)
        let params = parse_query_params(Some("name={invalid json")).unwrap();
        assert_eq!(
            params.filters.get("name").unwrap(),
            &serde_json::json!("{invalid json")
        );
    }

    #[test]
    fn test_map_db_error_to_router_error() {
        use in_mem_db_core::error::DbError;

        // Test client errors (should map to BadRequest)
        let client_errors = vec![
            DbError::TableNotFound {
                table: "test".to_string(),
            },
            DbError::FieldNotFound {
                table: "test".to_string(),
                field: "id".to_string(),
            },
            DbError::RecordNotFound {
                table: "test".to_string(),
                index: 0,
            },
            DbError::ProcedureNotFound {
                name: "test".to_string(),
            },
            DbError::FieldAlreadyExists {
                table: "test".to_string(),
                field: "id".to_string(),
            },
            DbError::CapacityOverflow { operation: "test" },
            DbError::TypeMismatch {
                expected: "u64".to_string(),
                got: "string".to_string(),
            },
            DbError::InvalidOffset {
                table: "test".to_string(),
                offset: 100,
                max: 50,
            },
            DbError::TableAlreadyExists("test".to_string()),
        ];

        for error in client_errors {
            let router_error = map_db_error_to_router_error(error.clone());
            match router_error {
                RouterError::BadRequest(_) => {
                    // Expected for non-404 errors
                    assert!(
                        error.to_string().contains("test") || error.to_string().contains("u64")
                    );
                    // Make sure it's not a TableNotFound or FieldNotFound
                    match error {
                        DbError::TableNotFound { .. } | DbError::FieldNotFound { .. } => {
                            panic!("Expected NotFound for {:?}, got BadRequest", error)
                        }
                        _ => {} // OK
                    }
                }
                RouterError::NotFound(_) => {
                    // Expected for TableNotFound, FieldNotFound, RecordNotFound, and ProcedureNotFound
                    match error {
                        DbError::TableNotFound { .. }
                        | DbError::FieldNotFound { .. }
                        | DbError::RecordNotFound { .. }
                        | DbError::ProcedureNotFound { .. } => {} // OK
                        _ => panic!("Expected BadRequest for {:?}, got NotFound", error),
                    }
                }
                _ => panic!(
                    "Expected BadRequest or NotFound for {:?}, got {:?}",
                    error, router_error
                ),
            }
        }

        // Test server errors (should map to InternalError)
        let server_errors = vec![
            DbError::SerializationError("test".to_string()),
            DbError::ProcedurePanic("test".to_string()),
            DbError::LockPoisoned,
            DbError::Timeout,
        ];

        for error in server_errors {
            let router_error = map_db_error_to_router_error(error.clone());
            match router_error {
                RouterError::InternalError(_) => {
                    // Expected
                    assert!(router_error.to_string().contains("Runtime error"));
                }
                _ => panic!(
                    "Expected InternalError for {:?}, got {:?}",
                    error, router_error
                ),
            }
        }
    }
}
