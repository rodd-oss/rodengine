//! Request utilities for HTTP endpoints.

use http_body_util::BodyExt;
use hyper::{body::Bytes, Request, Response};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::time;

use crate::router::RouterError;
use in_mem_db_runtime::QueryParams;

/// Type alias for matchit parameters with explicit lifetimes
pub type MatchitParams<'a, 'b> = matchit::Params<'a, 'b>;

/// Helper function to read request body with timeout
pub async fn read_request_body_with_timeout(
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
pub async fn wait_for_response_with_timeout<T>(
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
pub fn map_db_error_to_router_error(e: in_mem_db_core::error::DbError) -> RouterError {
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

/// Helper to build HTTP response with proper error handling
pub fn build_response(status: u16, json: Vec<u8>) -> Result<Response<Bytes>, RouterError> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Bytes::from(json))
        .map_err(|e| RouterError::InternalError(format!("Failed to build response: {}", e)))
}

/// Helper to build empty HTTP response (for 204 No Content)
pub fn build_empty_response(status: u16) -> Result<Response<Bytes>, RouterError> {
    Response::builder()
        .status(status)
        .body(Bytes::new())
        .map_err(|e| RouterError::InternalError(format!("Failed to build response: {}", e)))
}

/// Parse query parameters from URL query string.
pub fn parse_query_params(query_str: Option<&str>) -> Result<QueryParams, RouterError> {
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
