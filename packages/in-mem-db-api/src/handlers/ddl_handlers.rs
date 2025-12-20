//! DDL (Data Definition Language) operation handlers.

use hyper::{body::Bytes, Request, Response};

use crate::router::{AppState, RouterError};
use in_mem_db_core::table::Field;
use in_mem_db_runtime::ApiRequest;

use super::request_utils::{
    build_empty_response, build_response, map_db_error_to_router_error,
    read_request_body_with_timeout, wait_for_response_with_timeout, AddFieldRequest,
    AddFieldResponse, CreateRelationRequest, CreateRelationResponse, CreateTableRequest,
    CreateTableResponse,
};
use super::response::success_response;

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
    params: super::request_utils::MatchitParams<'_, '_>,
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
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::CreateTable {
        name: table_name.clone(),
        fields,
        response: tx,
    };
    // Send request via async channel
    state
        .api_tx
        .send(api_request)
        .await
        .map_err(|e| RouterError::InternalError(format!("Channel closed: {}", e)))?;

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
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::DeleteTable {
        name: table_name.clone(),
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
    params: super::request_utils::MatchitParams<'_, '_>,
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
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::AddField {
        table: table_name.clone(),
        field,
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
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let field_name = params.get("field").unwrap_or("unknown").to_string();

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::RemoveField {
        table: table_name.clone(),
        field_name: field_name.clone(),
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
    _params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateRelationRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::BadRequest(format!("Failed to parse request: {}", e)))?;

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::CreateRelation {
        from_table: request.from_table.clone(),
        from_field: request.from_field.clone(),
        to_table: request.to_table.clone(),
        to_field: request.to_field.clone(),
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
    params: super::request_utils::MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let relation_id = params.get("id").unwrap_or("unknown").to_string();

    // Send request to runtime
    let (tx, rx) = tokio::sync::oneshot::channel();
    let api_request = ApiRequest::DeleteRelation {
        id: relation_id.clone(),
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
