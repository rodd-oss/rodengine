//! API request types and implementations

use std::collections::HashMap;

use crate::ResponseSender;
use in_mem_db_core::table::Field;
use serde_json::Value;

/// API request from REST server
#[derive(Debug)]
pub enum ApiRequest {
    /// Create table
    CreateTable {
        name: String,
        fields: Vec<Field>,
        response: ResponseSender,
    },
    /// Delete table
    DeleteTable {
        name: String,
        response: ResponseSender,
    },
    /// Add field to table
    AddField {
        table: String,
        field: Field,
        response: ResponseSender,
    },
    /// Remove field from table
    RemoveField {
        table: String,
        field_name: String,
        response: ResponseSender,
    },
    /// Create relation between tables
    CreateRelation {
        from_table: String,
        from_field: String,
        to_table: String,
        to_field: String,
        response: ResponseSender,
    },
    /// Delete relation
    DeleteRelation {
        id: String,
        response: ResponseSender,
    },
    /// CRUD operation
    Crud {
        table: String,
        operation: CrudOperation,
        response: ResponseSender,
    },
    /// RPC call
    Rpc {
        name: String,
        params: Value,
        response: ResponseSender,
    },
    /// List all tables
    ListTables { response: ResponseSender },
    /// Query records with filtering and pagination
    QueryRecords {
        table: String,
        query: QueryParams,
        response: ResponseSender,
    },
}

impl ApiRequest {
    /// Returns true if this request is a DDL (Data Definition Language) operation.
    pub fn is_ddl(&self) -> bool {
        match self {
            ApiRequest::CreateTable { .. } => true,
            ApiRequest::DeleteTable { .. } => true,
            ApiRequest::AddField { .. } => true,
            ApiRequest::RemoveField { .. } => true,
            ApiRequest::CreateRelation { .. } => true,
            ApiRequest::DeleteRelation { .. } => true,
            ApiRequest::Crud { .. } => false,
            ApiRequest::Rpc { .. } => false,
            ApiRequest::ListTables { .. } => false,
            ApiRequest::QueryRecords { .. } => false,
        }
    }
}

/// Query parameters for filtering and pagination
#[derive(Debug, Clone)]
pub struct QueryParams {
    /// Maximum number of records to return
    pub limit: Option<usize>,
    /// Number of records to skip
    pub offset: Option<usize>,
    /// Field equality filters (field_name -> value)
    pub filters: HashMap<String, Value>,
}

/// CRUD operation types
#[derive(Debug)]
pub enum CrudOperation {
    Create { values: Vec<Value> },
    Read { id: u64 },
    Update { id: u64, values: Vec<Value> },
    Delete { id: u64 },
    Query { query: QueryParams },
}

/// Procedure call
#[derive(Debug)]
pub struct ProcedureCall {
    /// Procedure name
    pub name: String,
    /// JSON parameters
    pub params: Value,
    /// Transaction handle for procedure isolation
    pub tx_handle: in_mem_db_core::transaction::TransactionHandle,
    /// Response sender to send result back to API caller
    pub response: Option<ResponseSender>,
}

/// Runtime tick phases
#[derive(Debug, Clone, Copy)]
pub enum TickPhase {
    /// API request processing (30% of tick)
    Api,
    /// Procedure execution (50% of tick)
    Procedures,
    /// Persistence (20% of tick)
    Persistence,
}

/// Persistence flush command
#[derive(Debug)]
pub enum FlushCommand {
    /// Flush all tables
    FlushAll,
    /// Flush specific table
    FlushTable(String),
}
