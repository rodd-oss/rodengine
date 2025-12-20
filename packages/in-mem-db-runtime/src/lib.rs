//! Tick-based runtime and procedure execution system

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::error::DbError;
use in_mem_db_core::persistence;
use tokio::sync::oneshot;

mod procedure;

pub use procedure::{ProcedureFn, ProcedureRegistry};

/// Result type for runtime operations
pub type Result<T> = std::result::Result<T, DbError>;

/// Response sender for API requests
pub type ResponseSender = oneshot::Sender<Result<serde_json::Value>>;

/// API request from REST server
#[derive(Debug)]
pub enum ApiRequest {
    /// Create table
    CreateTable {
        name: String,
        fields: Vec<in_mem_db_core::table::Field>,
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
        field: in_mem_db_core::table::Field,
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
        params: serde_json::Value,
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
    pub filters: std::collections::HashMap<String, serde_json::Value>,
}

/// CRUD operation types
#[derive(Debug)]
pub enum CrudOperation {
    Create {
        values: Vec<serde_json::Value>,
    },
    Read {
        id: u64,
    },
    Update {
        id: u64,
        values: Vec<serde_json::Value>,
    },
    Delete {
        id: u64,
    },
    Query {
        query: QueryParams,
    },
}

/// Procedure call
#[derive(Debug)]
pub struct ProcedureCall {
    /// Procedure name
    pub name: String,
    /// JSON parameters
    pub params: serde_json::Value,
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

/// Main runtime loop
pub struct Runtime {
    /// Database instance
    database: Arc<Database>,
    /// Configuration
    config: DbConfig,
    /// Tick duration
    tick_duration: Duration,
    /// API request receiver
    api_rx: mpsc::Receiver<ApiRequest>,
    /// DDL request queue (higher priority)
    ddl_queue: VecDeque<ApiRequest>,
    /// DML request queue (lower priority)
    dml_queue: VecDeque<ApiRequest>,
    /// Procedure queue
    procedure_queue: VecDeque<ProcedureCall>,
    /// Persistence channel sender (kept for potential emergency flushes or future use)
    #[allow(dead_code)]
    persistence_tx: mpsc::SyncSender<FlushCommand>,
    /// Procedure registry
    procedure_registry: ProcedureRegistry,
    /// API requests processed in current tick
    api_requests_processed_this_tick: AtomicU32,
    /// Total dropped requests due to rate limiting
    dropped_requests: AtomicU64,
    /// Current queue size (estimated)
    queue_size: AtomicU64,
    /// Maximum queue capacity (tickrate * 100)
    queue_capacity: usize,
    /// Current tick count
    tick_count: u64,
}

/// Persistence flush command
#[derive(Debug)]
pub enum FlushCommand {
    /// Flush all tables
    FlushAll,
    /// Flush specific table
    FlushTable(String),
}

impl Runtime {
    /// Create a new runtime
    pub fn new(
        database: Arc<Database>,
        config: DbConfig,
        api_rx: mpsc::Receiver<ApiRequest>,
        persistence_tx: mpsc::SyncSender<FlushCommand>,
    ) -> Self {
        let tick_duration = Duration::from_secs_f64(1.0 / config.tickrate as f64);
        let queue_capacity = config.tickrate as usize * 100;
        Self {
            database,
            config,
            tick_duration,
            api_rx,
            ddl_queue: VecDeque::new(),
            dml_queue: VecDeque::new(),
            procedure_queue: VecDeque::new(),
            persistence_tx,
            procedure_registry: ProcedureRegistry::new(),
            api_requests_processed_this_tick: AtomicU32::new(0),
            dropped_requests: AtomicU64::new(0),
            queue_size: AtomicU64::new(0),
            queue_capacity,
            tick_count: 0,
        }
    }

    /// Registers a procedure with the runtime.
    ///
    /// # Arguments
    /// * `name` - Procedure name
    /// * `func` - Procedure function
    pub fn register_procedure(&mut self, name: String, func: ProcedureFn) {
        self.procedure_registry.register(name, func);
    }

    /// Get queue sizes for testing
    pub fn queue_sizes(&self) -> (usize, usize, usize) {
        (
            self.ddl_queue.len(),
            self.dml_queue.len(),
            self.procedure_queue.len(),
        )
    }

    /// Drain API channel into priority queues, respecting capacity.
    fn drain_api_channel(&mut self) {
        while let Ok(req) = self.api_rx.try_recv() {
            let current_size = self.queue_size.load(Ordering::Relaxed);
            if current_size >= self.queue_capacity as u64 {
                // Queue full, drop request
                self.dropped_requests.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            if req.is_ddl() {
                self.ddl_queue.push_back(req);
            } else {
                self.dml_queue.push_back(req);
            }
            self.queue_size.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Process queued API requests up to limit and time budget.
    fn process_queued_requests(
        &mut self,
        tick_start: Instant,
        time_budget: Duration,
    ) -> Result<()> {
        let max_requests = self.config.max_api_requests_per_tick;
        let mut processed = 0;

        // Process DDL requests first
        while processed < max_requests {
            if tick_start.elapsed() > time_budget {
                break;
            }
            if let Some(req) = self.ddl_queue.pop_front() {
                self.handle_api_request(req)?;
                processed += 1;
                self.queue_size.fetch_sub(1, Ordering::Relaxed);
            } else {
                break;
            }
        }

        // Then DML requests
        while processed < max_requests {
            if tick_start.elapsed() > time_budget {
                break;
            }
            if let Some(req) = self.dml_queue.pop_front() {
                self.handle_api_request(req)?;
                processed += 1;
                self.queue_size.fetch_sub(1, Ordering::Relaxed);
            } else {
                break;
            }
        }

        self.api_requests_processed_this_tick
            .store(processed, Ordering::Relaxed);
        Ok(())
    }

    /// Execute a single tick of the runtime
    pub fn tick(&mut self) -> Result<()> {
        let tick_start = Instant::now();

        // Phase 1: API requests
        self.process_api_phase(tick_start)?;

        // Phase 2: Procedures
        self.process_procedure_phase(tick_start)?;

        // Phase 3: Persistence
        self.process_persistence_phase()?;

        // Sleep remainder of tick
        self.sleep_remaining(tick_start);

        self.tick_count += 1;
        Ok(())
    }

    /// Run the runtime loop (blocking)
    pub fn run(&mut self) -> Result<()> {
        loop {
            self.tick()?;
        }
    }

    /// Process API phase (30% of tick)
    fn process_api_phase(&mut self, tick_start: Instant) -> Result<()> {
        let api_time_budget = self.tick_duration.mul_f32(0.3);

        // Drain incoming channel into priority queues
        self.drain_api_channel();

        // Process queued requests respecting time budget and rate limit
        self.process_queued_requests(tick_start, api_time_budget)?;

        Ok(())
    }

    /// Process procedure phase (50% of tick)
    fn process_procedure_phase(&mut self, tick_start: Instant) -> Result<()> {
        let procedure_time_budget = self.tick_duration.mul_f32(0.5);

        if self.procedure_queue.is_empty() {
            return Ok(());
        }

        // Use Rayon for parallel execution if available
        #[cfg(feature = "parallel")]
        {
            self.process_procedures_parallel(tick_start, procedure_time_budget)
        }
        #[cfg(not(feature = "parallel"))]
        {
            self.process_procedures_sequential(tick_start, procedure_time_budget)
        }
    }

    /// Process procedures sequentially
    #[allow(dead_code)]
    fn process_procedures_sequential(
        &mut self,
        tick_start: Instant,
        time_budget: Duration,
    ) -> Result<()> {
        let mut processed_count = 0;

        while let Some(call) = self.procedure_queue.pop_front() {
            if tick_start.elapsed() > time_budget {
                // Put the current call back in the queue for next tick
                self.procedure_queue.push_front(call);
                break;
            }

            // Run procedure - errors are sent through response channel
            let _ = self.run_procedure(call);
            processed_count += 1;
        }

        tracing::debug!("Processed {} procedures sequentially", processed_count);
        Ok(())
    }

    /// Process procedures in parallel using Rayon
    #[cfg(feature = "parallel")]
    fn process_procedures_parallel(
        &mut self,
        tick_start: Instant,
        time_budget: Duration,
    ) -> Result<()> {
        use rayon::prelude::*;

        // Collect calls to avoid borrow issues
        let calls: Vec<_> = self.procedure_queue.drain(..).collect();

        // Extract thread-safe references
        let db = self.database.clone();
        let registry = self.procedure_registry.clone();

        // Process calls in parallel with time budget check
        let start_time = tick_start;
        let results: Vec<std::result::Result<(), DbError>> = calls
            .into_par_iter()
            .map(|call| {
                // Check time budget before starting each procedure
                if start_time.elapsed() > time_budget {
                    // Send timeout error through response channel
                    if let Some(response) = call.response {
                        let _ = response.send(Err(DbError::Timeout));
                    }
                    return Err(DbError::Timeout);
                }

                // Run procedure with extracted references
                Self::run_procedure_with_refs(&db, &registry, call)
            })
            .collect();

        // Separate successful and failed results
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for result in results {
            match result {
                Ok(()) => successful.push(()),
                Err(e) => failed.push(e),
            }
        }

        // Log results
        tracing::debug!(
            "Parallel procedure execution: {} succeeded, {} failed",
            successful.len(),
            failed.len()
        );

        // If any procedure timed out, we need to handle it specially
        // For now, just return the first error
        if let Some(first_error) = failed.into_iter().next() {
            return Err(first_error);
        }

        Ok(())
    }

    /// Run a procedure with explicit references (for parallel execution)
    fn run_procedure_with_refs(
        db: &Arc<Database>,
        registry: &ProcedureRegistry,
        mut call: ProcedureCall,
    ) -> Result<()> {
        tracing::debug!(
            "Running procedure {} with params {:?}",
            call.name,
            call.params
        );

        // Validate parameters against schema (if any)
        if let Err(e) = registry.validate_params(&call.name, &call.params) {
            tracing::error!("Procedure {} parameter validation failed: {}", call.name, e);
            // Send error response if channel exists
            if let Some(response) = call.response {
                let _ = response.send(Err(e.clone()));
            }
            return Err(e);
        }

        // Look up procedure in registry
        let Some(proc_fn) = registry.get(&call.name) else {
            let error = DbError::ProcedureNotFound {
                name: call.name.clone(),
            };
            tracing::error!("{}", error);
            // Send error response if channel exists
            if let Some(response) = call.response {
                let _ = response.send(Err(error.clone()));
            }
            return Err(error);
        };

        // Execute procedure with panic catching
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            proc_fn(db, &mut call.tx_handle, call.params)
        }));

        let procedure_result = match result {
            Ok(Ok(proc_result)) => {
                // Procedure succeeded, commit the transaction
                match db.commit_transaction(&mut call.tx_handle) {
                    Ok(()) => {
                        tracing::debug!(
                            "Procedure {} completed and committed successfully",
                            call.name
                        );
                        Ok(proc_result)
                    }
                    Err(e) => {
                        tracing::error!("Procedure {} commit failed: {}", call.name, e);
                        Err(e)
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!("Procedure {} failed: {}", call.name, e);
                // Transaction will be aborted automatically when tx_handle drops
                Err(e)
            }
            Err(panic) => {
                let panic_msg = if let Some(msg) = panic.downcast_ref::<&str>() {
                    msg.to_string()
                } else if let Some(msg) = panic.downcast_ref::<String>() {
                    msg.clone()
                } else {
                    "unknown panic".to_string()
                };
                tracing::error!("Procedure {} panicked: {}", call.name, panic_msg);
                // Transaction will be aborted automatically when tx_handle drops
                Err(DbError::ProcedurePanic(panic_msg))
            }
        };

        // Send result back through response channel if it exists
        if let Some(response) = call.response {
            let _ = response.send(procedure_result);
        }

        // Return Ok(()) to indicate procedure was processed (not necessarily successful)
        Ok(())
    }

    /// Process persistence phase (20% of tick)
    fn process_persistence_phase(&mut self) -> Result<()> {
        // PersistenceManager.tick() handles periodic flushing in the background thread
        // We keep this phase for potential future persistence-related operations
        // or for sending emergency flush commands if needed
        Ok(())
    }

    /// Sleep remaining tick time
    fn sleep_remaining(&self, tick_start: Instant) {
        if let Some(remaining) = self.tick_duration.checked_sub(tick_start.elapsed()) {
            std::thread::sleep(remaining);
        }
    }

    /// Handle API request
    fn handle_api_request(&mut self, req: ApiRequest) -> Result<()> {
        match req {
            ApiRequest::CreateTable {
                name,
                fields,
                response,
            } => {
                tracing::info!("Creating table {} with {} fields", name, fields.len());
                let result = self
                    .database
                    .create_table(name.clone(), fields, None, self.config.max_buffer_size)
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
                let call = ProcedureCall {
                    name,
                    params,
                    tx_handle,
                    response: Some(response),
                };

                self.procedure_queue.push_back(call);
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
    fn json_value_to_bytes(&self, value: &serde_json::Value, field_type: &str) -> Result<Vec<u8>> {
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
    fn bytes_to_json_value(&self, bytes: &[u8], field_type: &str) -> Result<serde_json::Value> {
        match field_type {
            "u8" => Ok(serde_json::Value::Number((bytes[0] as u64).into())),
            "u16" => {
                let val =
                    u16::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "u16 (2 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(serde_json::Value::Number(val.into()))
            }
            "u32" => {
                let val =
                    u32::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "u32 (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(serde_json::Value::Number(val.into()))
            }
            "u64" => {
                let val =
                    u64::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "u64 (8 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(serde_json::Value::Number(val.into()))
            }
            "i8" => Ok(serde_json::Value::Number((bytes[0] as i8 as i64).into())),
            "i16" => {
                let val =
                    i16::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "i16 (2 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(serde_json::Value::Number(val.into()))
            }
            "i32" => {
                let val =
                    i32::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "i32 (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(serde_json::Value::Number(val.into()))
            }
            "i64" => {
                let val =
                    i64::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "i64 (8 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                Ok(serde_json::Value::Number(val.into()))
            }
            "f32" => {
                let val =
                    f32::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "f32 (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                match serde_json::Number::from_f64(val as f64) {
                    Some(num) => Ok(serde_json::Value::Number(num)),
                    None => Ok(serde_json::Value::Null), // Handle NaN/infinity as null
                }
            }
            "f64" => {
                let val =
                    f64::from_le_bytes(bytes.try_into().map_err(|_| DbError::TypeMismatch {
                        expected: "f64 (8 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    })?);
                match serde_json::Number::from_f64(val) {
                    Some(num) => Ok(serde_json::Value::Number(num)),
                    None => Ok(serde_json::Value::Null), // Handle NaN/infinity as null
                }
            }
            "bool" => Ok(serde_json::Value::Bool(bytes[0] != 0)),
            "string" => {
                let len = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
                    DbError::TypeMismatch {
                        expected: "string length (4 bytes)".to_string(),
                        got: format!("{} bytes", bytes.len()),
                    }
                })?) as usize;
                let str_bytes = &bytes[4..4 + len];
                Ok(serde_json::Value::String(
                    String::from_utf8_lossy(str_bytes).to_string(),
                ))
            }
            _ => {
                // For custom types, return as hex string
                Ok(serde_json::Value::String(hex::encode(bytes)))
            }
        }
    }

    /// Handle create record operation
    fn handle_create_record(
        &self,
        table: &str,
        values: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
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
    fn handle_read_record(&self, table: &str, id: u64) -> Result<serde_json::Value> {
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
        Ok(serde_json::Value::Object(result))
    }

    /// Handle update record operation
    fn handle_update_record(
        &self,
        table: &str,
        id: u64,
        values: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
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
        Ok(serde_json::Value::Null)
    }

    /// Handle delete record operation
    fn handle_delete_record(&self, table: &str, id: u64) -> Result<serde_json::Value> {
        let record_index = id as usize - 1; // Convert ID to 0-based index
        let table_ref = self.database.get_table_mut(table)?;
        table_ref.delete_record(record_index, "runtime")?;
        Ok(serde_json::Value::Null)
    }

    /// Handle create relation operation
    fn handle_create_relation(
        &self,
        from_table: &str,
        from_field: &str,
        to_table: &str,
        to_field: &str,
    ) -> Result<serde_json::Value> {
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
    fn handle_delete_relation(&self, relation_id: &str) -> Result<serde_json::Value> {
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

        Ok(serde_json::Value::Null)
    }

    /// Handle list tables operation
    fn handle_list_tables(&self) -> Result<serde_json::Value> {
        let table_names = self.database.table_names();
        let count = table_names.len();
        Ok(serde_json::json!({
            "tables": table_names,
            "count": count,
        }))
    }

    /// Handle query records operation
    fn handle_query_records(&self, table: &str, query: QueryParams) -> Result<serde_json::Value> {
        let table_ref = self.database.get_table(table)?;

        // Convert JSON filters to byte filters
        let mut byte_filters = std::collections::HashMap::new();
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
                serde_json::Value::Number((record_index as u64 + 1).into()),
            );

            matching_records.push(serde_json::Value::Object(record_obj));
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

    /// Run a procedure
    pub fn run_procedure(&self, call: ProcedureCall) -> Result<()> {
        Self::run_procedure_with_refs(&self.database, &self.procedure_registry, call)
    }
}
