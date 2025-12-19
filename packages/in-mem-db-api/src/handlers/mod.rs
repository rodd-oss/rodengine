//! HTTP endpoint implementations for CRUD, DDL, and RPC.

use http_body_util::BodyExt;
use hyper::{body::Bytes, Request, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::time;

use crate::router::{AppState, RouterError};
use in_mem_db_core::table::Field;
use in_mem_db_runtime::{ApiRequest, CrudOperation};

/// Type alias for matchit parameters with explicit lifetimes
type MatchitParams<'a, 'b> = matchit::Params<'a, 'b>;

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
pub async fn create_table(
    req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Read and parse request body with timeout
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateTableRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::InternalError(format!("Failed to parse request: {}", e)))?;

    // Create fields
    let type_registry = state.db.type_registry();
    let mut fields = Vec::new();

    for field_def in request.fields {
        let layout = type_registry.get(&field_def.r#type).ok_or_else(|| {
            RouterError::InternalError(format!("Unknown type: {}", field_def.r#type))
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

    let response_json =
        result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    // Convert to CreateTableResponse
    let record_size = response_json
        .get("record_size")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let response = CreateTableResponse {
        table: table_name,
        record_size: record_size as usize,
    };

    let json = serde_json::to_vec(&response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    Ok(Response::builder()
        .status(201)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .unwrap())
}

/// Deletes a table.
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

    Ok(Response::builder().status(204).body(Bytes::new()).unwrap())
}

/// Adds a field to an existing table.
pub async fn add_field(
    req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: AddFieldRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::InternalError(format!("Failed to parse request: {}", e)))?;

    // Get type layout
    let type_registry = state.db.type_registry();
    let layout = type_registry
        .get(&request.r#type)
        .ok_or_else(|| RouterError::InternalError(format!("Unknown type: {}", request.r#type)))?;

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

    let response_json =
        result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    let offset = response_json
        .get("offset")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let record_size = response_json
        .get("record_size")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let response = AddFieldResponse {
        offset: offset as usize,
        record_size: record_size as usize,
    };

    let json = serde_json::to_vec(&response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .unwrap())
}

/// Removes a field from a table.
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

    Ok(Response::builder().status(204).body(Bytes::new()).unwrap())
}

/// Creates a new record in a table.
pub async fn create_record(
    req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateRecordRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::InternalError(format!("Failed to parse request: {}", e)))?;

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

    let response_json =
        result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    // Expect response contains "id": u64
    let id = response_json
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?;
    let response = CreateRecordResponse { id };

    let json = serde_json::to_vec(&response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    Ok(Response::builder()
        .status(201)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .unwrap())
}

/// Reads a record from a table.
pub async fn read_record(
    _req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::InternalError(format!("Invalid record ID '{}': {}", record_id_str, e))
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

    let response_json =
        result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    // Return as JSON
    let json = serde_json::to_vec(&response_json)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .unwrap())
}

/// Fully updates a record.
pub async fn update_record(
    req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::InternalError(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: UpdateRecordRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::InternalError(format!("Failed to parse request: {}", e)))?;

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

    Ok(Response::builder().status(204).body(Bytes::new()).unwrap())
}

/// Partially updates a record.
pub async fn partial_update_record(
    req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::InternalError(format!("Invalid record ID '{}': {}", record_id_str, e))
    })?;

    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: PartialUpdateRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::InternalError(format!("Failed to parse request: {}", e)))?;

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
            .map_err(|e| RouterError::InternalError(format!("Failed to get table: {}", e)))?;

        for field in &table_ref.fields {
            if let Some(update_value) = request.updates.get(&field.name) {
                updated_values.push(update_value.clone());
            } else if let Some(current_value) = current_obj.get(&field.name) {
                updated_values.push(current_value.clone());
            } else {
                return Err(RouterError::InternalError(format!(
                    "Field {} not found in current record",
                    field.name
                )));
            }
        }
    } else {
        return Err(RouterError::InternalError(
            "Invalid record format".to_string(),
        ));
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

    Ok(Response::builder().status(204).body(Bytes::new()).unwrap())
}

/// Deletes a record.
pub async fn delete_record(
    _req: Request<hyper::body::Incoming>,
    params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    let table_name = params.get("name").unwrap_or("unknown").to_string();
    let record_id_str = params.get("id").unwrap_or("0");

    let record_id: u64 = record_id_str.parse().map_err(|e| {
        RouterError::InternalError(format!("Invalid record ID '{}': {}", record_id_str, e))
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

    Ok(Response::builder().status(204).body(Bytes::new()).unwrap())
}

/// Creates a relation between tables.
pub async fn create_relation(
    req: Request<hyper::body::Incoming>,
    _params: MatchitParams<'_, '_>,
    state: AppState,
) -> Result<Response<Bytes>, RouterError> {
    // Read and parse request body
    let body_bytes = read_request_body_with_timeout(req, state.config.request_timeout_ms).await?;

    let request: CreateRelationRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| RouterError::InternalError(format!("Failed to parse request: {}", e)))?;

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

    let response_json =
        result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    // Extract relation ID from response
    let id = response_json
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RouterError::InternalError("Invalid response from runtime".to_string()))?
        .to_string();

    let response = CreateRelationResponse { id };

    let json = serde_json::to_vec(&response)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    Ok(Response::builder()
        .status(201)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .unwrap())
}

/// Deletes a relation.
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

    Ok(Response::builder().status(204).body(Bytes::new()).unwrap())
}

/// Executes a registered procedure via RPC.
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
        serde_json::from_slice(&body_bytes).map_err(|e| {
            RouterError::InternalError(format!("Failed to parse JSON params: {}", e))
        })?
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

    let response_json =
        result.map_err(|e| RouterError::InternalError(format!("Runtime error: {}", e)))?;

    let json = serde_json::to_vec(&response_json)
        .map_err(|e| RouterError::InternalError(format!("Failed to serialize response: {}", e)))?;

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .unwrap())
}
