//! API request handlers

use std::collections::HashMap;
use std::sync::Arc;

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::error::DbError;
use in_mem_db_core::persistence;
use serde_json::Value;

use crate::api_request::{ApiRequest, CrudOperation, QueryParams};
use crate::Result;

/// API request handlers
pub struct ApiHandlers {
    /// Database instance
    database: Arc<Database>,
    /// Configuration
    config: DbConfig,
    /// Procedure queue sender (for RPC calls)
    procedure_queue_sender: Option<std::sync::mpsc::Sender<crate::api_request::ProcedureCall>>,
}

impl ApiHandlers {
    /// Create new API handlers
    pub fn new(database: Arc<Database>, config: DbConfig) -> Self {
        Self {
            database,
            config,
            procedure_queue_sender: None,
        }
    }

    /// Set procedure queue sender for RPC calls
    pub fn set_procedure_queue_sender(
        &mut self,
        sender: std::sync::mpsc::Sender<crate::api_request::ProcedureCall>,
    ) {
        self.procedure_queue_sender = Some(sender);
    }

    /// Handle API request
    pub fn handle_api_request(&self, req: ApiRequest) -> Result<()> {
        match req {
            ApiRequest::CreateTable {
                name,
                fields,
                response,
            } => {
                tracing::info!("Creating table {} with {} fields", name, fields.len());
                let result = self
                    .database
                    .create_table(
                        name.clone(),
                        fields,
                        Some(self.config.initial_table_capacity),
                        self.config.max_buffer_size,
                    )
                    .and_then(|()| {
                        let table = self.database.get_table(&name)?;
                        Ok(serde_json::json!({
                            "table": name,
                            "record_size": table.record_size,
                        }))
                    });
                // Save schema after DDL
                if result.is_ok() {
                    if let Err(e) = persistence::save_schema_after_ddl(&self.database, &self.config)
                    {
                        tracing::error!("Failed to save schema: {}", e);
                    }
                }
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::DeleteTable { name, response } => {
                tracing::info!("Deleting table {}", name);
                let result = self
                    .database
                    .delete_table(&name)
                    .map(|()| serde_json::Value::Null);
                if result.is_ok() {
                    if let Err(e) = persistence::save_schema_after_ddl(&self.database, &self.config)
                    {
                        tracing::error!("Failed to save schema: {}", e);
                    }
                }
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::AddField {
                table,
                field,
                response,
            } => {
                tracing::info!("Adding field {} to table {}", field.name, table);
                let result = self
                    .database
                    .get_table_mut(&table)
                    .and_then(|mut table_ref| {
                        let offset = table_ref.add_field(
                            field.name.clone(),
                            field.type_id.clone(),
                            field.layout.clone(),
                        )?;
                        Ok(serde_json::json!({
                            "offset": offset,
                            "record_size": table_ref.record_size,
                        }))
                    });
                if result.is_ok() {
                    if let Err(e) = persistence::save_schema_after_ddl(&self.database, &self.config)
                    {
                        tracing::error!("Failed to save schema: {}", e);
                    }
                }
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::RemoveField {
                table,
                field_name,
                response,
            } => {
                tracing::info!("Removing field {} from table {}", field_name, table);
                let result = self
                    .database
                    .get_table_mut(&table)
                    .and_then(|mut table_ref| {
                        table_ref.remove_field(&field_name)?;
                        Ok(serde_json::Value::Null)
                    });
                if result.is_ok() {
                    if let Err(e) = persistence::save_schema_after_ddl(&self.database, &self.config)
                    {
                        tracing::error!("Failed to save schema: {}", e);
                    }
                }
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::CreateRelation {
                from_table,
                from_field,
                to_table,
                to_field,
                response,
            } => {
                tracing::info!(
                    "Creating relation from {}.{} to {}.{}",
                    from_table,
                    from_field,
                    to_table,
                    to_field
                );
                let result =
                    self.handle_create_relation(&from_table, &from_field, &to_table, &to_field);
                if result.is_ok() {
                    if let Err(e) = persistence::save_schema_after_ddl(&self.database, &self.config)
                    {
                        tracing::error!("Failed to save schema: {}", e);
                    }
                }
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::DeleteRelation { id, response } => {
                tracing::info!("Deleting relation {}", id);
                let result = self.handle_delete_relation(&id);
                if result.is_ok() {
                    if let Err(e) = persistence::save_schema_after_ddl(&self.database, &self.config)
                    {
                        tracing::error!("Failed to save schema: {}", e);
                    }
                }
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::Crud {
                table,
                operation,
                response,
            } => {
                tracing::debug!("CRUD operation on table {}: {:?}", table, operation);
                let result = match operation {
                    CrudOperation::Create { values } => self.handle_create_record(&table, values),
                    CrudOperation::Read { id } => self.handle_read_record(&table, id),
                    CrudOperation::Update { id, values } => {
                        self.handle_update_record(&table, id, values)
                    }
                    CrudOperation::Delete { id } => self.handle_delete_record(&table, id),
                    CrudOperation::Query { query } => self.handle_query_records(&table, query),
                };
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::ListTables { response } => {
                tracing::debug!("Listing all tables");
                let result = self.handle_list_tables();
                let _ = response.send(result);
                Ok(())
            }
            ApiRequest::Rpc {
                name,
                params,
                response,
            } => {
                tracing::debug!("RPC call {} with params {:?}", name, params);
                // Create a transaction handle for procedure isolation
                let tx_handle = in_mem_db_core::transaction::TransactionHandle::new();

                // Create procedure call with response channel
                let call = crate::api_request::ProcedureCall {
                    name,
                    params,
                    tx_handle,
                    response: Some(response),
                };

                // Add to procedure queue
                if let Some(sender) = &self.procedure_queue_sender {
                    sender.send(call).map_err(|e| {
                        in_mem_db_core::error::DbError::SerializationError(e.to_string())
                    })?;
                } else {
                    tracing::error!("Procedure queue sender not set, cannot handle RPC call");
                    return Err(in_mem_db_core::error::DbError::SerializationError(
                        "Procedure queue not available".to_string(),
                    ));
                }
                Ok(())
            }
            ApiRequest::QueryRecords {
                table,
                query,
                response,
            } => {
                tracing::debug!("Query records from table {}: {:?}", table, query);
                let result = self.handle_query_records(&table, query);
                let _ = response.send(result);
                Ok(())
            }
        }
    }

    /// Convert JSON value to bytes based on field type
    fn json_value_to_bytes(&self, value: &Value, field_type: &str) -> Result<Vec<u8>> {
        match field_type {
            "u8" => {
                let num = value.as_u64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "u8".to_string(),
                    got: value.to_string(),
                })? as u8;
                Ok(vec![num])
            }
            "u16" => {
                let num = value.as_u64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "u16".to_string(),
                    got: value.to_string(),
                })? as u16;
                Ok(num.to_le_bytes().to_vec())
            }
            "u32" => {
                let num = value.as_u64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "u32".to_string(),
                    got: value.to_string(),
                })? as u32;
                Ok(num.to_le_bytes().to_vec())
            }
            "u64" => {
                let num = value.as_u64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "u64".to_string(),
                    got: value.to_string(),
                })?;
                Ok(num.to_le_bytes().to_vec())
            }
            "i8" => {
                let num = value.as_i64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "i8".to_string(),
                    got: value.to_string(),
                })? as i8;
                Ok(vec![num as u8])
            }
            "i16" => {
                let num = value.as_i64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "i16".to_string(),
                    got: value.to_string(),
                })? as i16;
                Ok(num.to_le_bytes().to_vec())
            }
            "i32" => {
                let num = value.as_i64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "i32".to_string(),
                    got: value.to_string(),
                })? as i32;
                Ok(num.to_le_bytes().to_vec())
            }
            "i64" => {
                let num = value.as_i64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "i64".to_string(),
                    got: value.to_string(),
                })?;
                Ok(num.to_le_bytes().to_vec())
            }
            "f32" => {
                let num = value.as_f64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "f32".to_string(),
                    got: value.to_string(),
                })? as f32;
                Ok(num.to_le_bytes().to_vec())
            }
            "f64" => {
                let num = value.as_f64().ok_or_else(|| DbError::TypeMismatch {
                    expected: "f64".to_string(),
                    got: value.to_string(),
                })?;
                Ok(num.to_le_bytes().to_vec())
            }
            "bool" => {
                let b = value.as_bool().ok_or_else(|| DbError::TypeMismatch {
                    expected: "bool".to_string(),
                    got: value.to_string(),
                })?;
                Ok(vec![if b { 1 } else { 0 }])
            }
            "string" => {
                let s = value.as_str().ok_or_else(|| DbError::TypeMismatch {
                    expected: "string".to_string(),
                    got: value.to_string(),
                })?;
                let bytes = s.as_bytes();
                let len = bytes.len() as u32;
                let mut result = len.to_le_bytes().to_vec();
                result.extend_from_slice(bytes);
                // Pad to 260 bytes (4-byte length + 256 bytes string data)
                result.resize(260, 0);
                Ok(result)
            }
            _ => {
                // For custom types, try to parse as hex string
                if let Some(s) = value.as_str() {
                    hex::decode(s).map_err(|e| DbError::SerializationError(e.to_string()))
                } else {
                    Err(DbError::TypeMismatch {
                        expected: "hex string".to_string(),
                        got: value.to_string(),
                    })
                }
            }
        }
    }

    /// Convert bytes to JSON value based on field type
    fn bytes_to_json_value(&self, bytes: &[u8], field_type: &str) -> Result<Value> {
        match field_type {
            "u8" => Ok(Value::Number((bytes[0] as u64).into())),
            "u16" => {
                let val =
                    u16::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "u16 (2 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(Value::Number(val.into()))
            }
            "u32" => {
                let val =
                    u32::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "u32 (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(Value::Number(val.into()))
            }
            "u64" => {
                let val =
                    u64::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "u64 (8 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(Value::Number(val.into()))
            }
            "i8" => Ok(Value::Number((bytes[0] as i8 as i64).into())),
            "i16" => {
                let val =
                    i16::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "i16 (2 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(Value::Number(val.into()))
            }
            "i32" => {
                let val =
                    i32::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "i32 (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(Value::Number(val.into()))
            }
            "i64" => {
                let val =
                    i64::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "i64 (8 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(Value::Number(val.into()))
            }
            "f32" => {
                let val =
                    f32::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "f32 (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                match serde_json::Number::from_f64(val as f64) {
                    Some(num) => Ok(Value::Number(num)),
                    None => Ok(Value::Null), // Handle NaN/infinity as null
                }
            }
            "f64" => {
                let val =
                    f64::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "f64 (8 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                match serde_json::Number::from_f64(val) {
                    Some(num) => Ok(Value::Number(num)),
                    None => Ok(Value::Null), // Handle NaN/infinity as null
                }
            }
            "bool" => Ok(Value::Bool(bytes[0] != 0)),
            "string" => {
                let len = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
                    DbError::TypeMismatch {
                        expected: "string length (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    }
                })?) as usize;
                let str_bytes = &bytes[4..4 + len];
                Ok(Value::String(
                    String::from_utf8_lossy(str_bytes).to_string(),
                ))
            }
            _ => {
                // For custom types, return as hex string
                Ok(Value::String(hex::encode(bytes)))
            }
        }
    }

    /// Handle create record operation
    fn handle_create_record(&self, table: &str, values: Vec<Value>) -> Result<Value> {
        let table_ref = self.database.get_table(table)?;

        // Convert JSON values to bytes
        let mut field_bytes = Vec::new();
        for (field, value) in table_ref.fields.iter().zip(values.iter()) {
            let bytes = self.json_value_to_bytes(value, &field.type_id)?;
            if bytes.len() != field.size {
                return Err(DbError::TypeMismatch {
                    expected: format!("{} bytes for field {}", field.size, field.name),
                    got: format!("{} bytes", bytes.len()),
                });
            }
            field_bytes.push(bytes);
        }

        // Create record
        let field_refs: Vec<&[u8]> = field_bytes.iter().map(|b| b.as_slice()).collect();
        let id = table_ref.create_record_from_values(&field_refs)?;
        Ok(serde_json::json!({ "id": id }))
    }

    /// Handle read record operation
    fn handle_read_record(&self, table: &str, id: u64) -> Result<Value> {
        let table_ref = self.database.get_table(table)?;
        let record_index = id as usize - 1; // Convert ID to 0-based index
        let (record_bytes, _arc) = table_ref
            .read_record(record_index)
            .map_err(|e| DbError::SerializationError(e.to_string()))?;

        // Convert bytes to JSON representation
        let mut result = serde_json::Map::new();
        for field in &table_ref.fields {
            let offset = field.offset;
            let field_bytes = &record_bytes[offset..offset + field.size];

            // Convert bytes to JSON value
            let value = self.bytes_to_json_value(field_bytes, &field.type_id)?;
            result.insert(field.name.clone(), value);
        }
        Ok(Value::Object(result))
    }

    /// Handle update record operation
    fn handle_update_record(&self, table: &str, id: u64, values: Vec<Value>) -> Result<Value> {
        let table_ref = self.database.get_table(table)?;
        let record_index = id as usize - 1; // Convert ID to 0-based index

        // Convert JSON values to bytes and create full record
        let mut record_bytes = vec![0u8; table_ref.record_size];
        for (field, value) in table_ref.fields.iter().zip(values.iter()) {
            let bytes = self.json_value_to_bytes(value, &field.type_id)?;
            if bytes.len() != field.size {
                return Err(DbError::TypeMismatch {
                    expected: format!("{} bytes for field {}", field.size, field.name),
                    got: format!("{} bytes", bytes.len()),
                });
            }
            let offset = field.offset;
            record_bytes[offset..offset + field.size].copy_from_slice(&bytes);
        }

        // Update record
        let table_ref_mut = self.database.get_table_mut(table)?;
        table_ref_mut.update_record(record_index, &record_bytes)?;
        Ok(Value::Null)
    }

    /// Handle delete record operation
    fn handle_delete_record(&self, table: &str, id: u64) -> Result<Value> {
        let record_index = id as usize - 1; // Convert ID to 0-based index
        let table_ref = self.database.get_table_mut(table)?;
        table_ref.delete_record(record_index, "runtime")?;
        Ok(Value::Null)
    }

    /// Handle create relation operation
    fn handle_create_relation(
        &self,
        from_table: &str,
        from_field: &str,
        to_table: &str,
        to_field: &str,
    ) -> Result<Value> {
        // Validate that both tables exist
        let from_table_ref = self.database.get_table(from_table)?;
        let to_table_ref = self.database.get_table(to_table)?;

        // Validate that fields exist in both tables
        let from_field_exists = from_table_ref.fields.iter().any(|f| f.name == from_field);
        let to_field_exists = to_table_ref.fields.iter().any(|f| f.name == to_field);

        if !from_field_exists {
            return Err(DbError::FieldNotFound {
                table: from_table.to_string(),
                field: from_field.to_string(),
            });
        }

        if !to_field_exists {
            return Err(DbError::FieldNotFound {
                table: to_table.to_string(),
                field: to_field.to_string(),
            });
        }

        // Create and add the relation
        let relation = in_mem_db_core::table::Relation {
            to_table: to_table.to_string(),
            from_field: from_field.to_string(),
            to_field: to_field.to_string(),
        };

        let mut from_table_ref = self.database.get_table_mut(from_table)?;
        from_table_ref.add_relation(relation);

        // Generate a relation ID (simple hash of the relation properties)
        let relation_id = format!(
            "rel_{}_{}_{}_{}",
            from_table, from_field, to_table, to_field
        );

        Ok(serde_json::json!({ "id": relation_id }))
    }

    /// Handle delete relation operation
    fn handle_delete_relation(&self, relation_id: &str) -> Result<Value> {
        // Parse relation ID to extract table and field information
        // Format: rel_{from_table}_{from_field}_{to_table}_{to_field}
        if !relation_id.starts_with("rel_") {
            return Err(DbError::SerializationError(
                "Invalid relation ID format".to_string(),
            ));
        }

        let parts: Vec<&str> = relation_id[4..].split('_').collect();
        if parts.len() != 4 {
            return Err(DbError::SerializationError(
                "Invalid relation ID format".to_string(),
            ));
        }

        let from_table = parts[0];
        let _from_field = parts[1];
        let to_table = parts[2];
        let _to_field = parts[3];

        // Remove the relation from the source table
        let mut table_ref = self.database.get_table_mut(from_table)?;
        let removed = table_ref.remove_relation(to_table);

        if !removed {
            return Err(DbError::SerializationError(format!(
                "Relation not found: {}",
                relation_id
            )));
        }

        Ok(Value::Null)
    }

    /// Handle list tables operation
    fn handle_list_tables(&self) -> Result<Value> {
        let table_names = self.database.table_names();
        let count = table_names.len();
        Ok(serde_json::json!({
            "tables": table_names,
            "count": count,
        }))
    }

    /// Handle query records operation
    fn handle_query_records(&self, table: &str, query: QueryParams) -> Result<Value> {
        let table_ref = self.database.get_table(table)?;

        // Convert JSON filters to byte filters
        let mut byte_filters = HashMap::new();
        for (field_name, filter_value) in &query.filters {
            if let Some(field) = table_ref.get_field(field_name) {
                let bytes = self.json_value_to_bytes(filter_value, &field.type_id)?;
                byte_filters.insert(field_name.clone(), bytes);
            } else {
                return Err(DbError::FieldNotFound {
                    table: table.to_string(),
                    field: field_name.clone(),
                });
            }
        }

        // Use efficient query method
        let matching_indices = table_ref.query_records(&byte_filters, query.limit, query.offset)?;

        // Convert matching records to JSON
        let mut matching_records = Vec::new();
        for record_index in matching_indices {
            // Read the full record
            let (record_bytes, _arc) = table_ref
                .read_record(record_index)
                .map_err(|e| DbError::SerializationError(e.to_string()))?;

            // Convert record to JSON representation
            let mut record_obj = serde_json::Map::new();
            for field in &table_ref.fields {
                let offset = field.offset;
                let field_bytes = &record_bytes[offset..offset + field.size];
                let value = self.bytes_to_json_value(field_bytes, &field.type_id)?;
                record_obj.insert(field.name.clone(), value);
            }

            // Add record ID (index + 1)
            record_obj.insert(
                "_id".to_string(),
                Value::Number((record_index as u64 + 1).into()),
            );

            matching_records.push(Value::Object(record_obj));
        }

        let total_records = table_ref.record_count();

        Ok(serde_json::json!({
            "records": matching_records,
            "count": matching_records.len(),
            "total": total_records,
            "limit": query.limit,
            "offset": query.offset,
        }))
    }
}
