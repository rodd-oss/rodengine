//! Runtime loop with tick phases and timing enforcement.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use in_mem_db_core::config::DbConfig;
use in_mem_db_core::database::Database;
use in_mem_db_core::error::DbError;
use tokio::sync::mpsc;

use crate::api_handlers::ApiHandlers;
use crate::api_request::{ApiRequest, FlushCommand, ProcedureCall};
use crate::procedure::ProcedureRegistry;
use crate::Result;

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
    persistence_tx: mpsc::Sender<FlushCommand>,
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
    /// API handlers
    api_handlers: ApiHandlers,
    /// Procedure queue receiver
    procedure_rx: std::sync::mpsc::Receiver<ProcedureCall>,
}

impl Runtime {
    /// Create a new runtime
    pub fn new(
        database: Arc<Database>,
        config: DbConfig,
        api_rx: mpsc::Receiver<ApiRequest>,
        persistence_tx: mpsc::Sender<FlushCommand>,
    ) -> Self {
        let tick_duration = Duration::from_secs_f64(1.0 / config.tickrate as f64);
        let queue_capacity = config.tickrate as usize * 100;
        let (procedure_tx, procedure_rx) = std::sync::mpsc::channel();
        let mut api_handlers = ApiHandlers::new(database.clone());
        api_handlers.set_procedure_queue_sender(procedure_tx);

        Self {
            database: database.clone(),
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
            api_handlers,
            procedure_rx,
        }
    }

    /// Registers a procedure with the runtime.
    ///
    /// # Arguments
    /// * `name` - Procedure name
    /// * `func` - Procedure function
    pub fn register_procedure(&mut self, name: String, func: crate::ProcedureFn) {
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

    /// Drain procedure channel into procedure queue.
    fn drain_procedure_channel(&mut self) {
        while let Ok(call) = self.procedure_rx.try_recv() {
            self.procedure_queue.push_back(call);
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
                self.api_handlers.handle_api_request(req)?;
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
                self.api_handlers.handle_api_request(req)?;
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
    pub fn process_api_phase(&mut self, tick_start: Instant) -> Result<()> {
        let api_time_budget = self.tick_duration.mul_f32(0.3);

        // Drain incoming channel into priority queues
        self.drain_api_channel();

        // Process queued requests respecting time budget and rate limit
        self.process_queued_requests(tick_start, api_time_budget)?;

        Ok(())
    }

    /// Process procedure phase (50% of tick)
    pub fn process_procedure_phase(&mut self, tick_start: Instant) -> Result<()> {
        let procedure_time_budget = self.tick_duration.mul_f32(0.5);

        // Drain any new procedure calls from channel
        self.drain_procedure_channel();

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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
    pub fn process_persistence_phase(&mut self) -> Result<()> {
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

    /// Run a procedure
    pub fn run_procedure(&self, call: ProcedureCall) -> Result<()> {
        Self::run_procedure_with_refs(&self.database, &self.procedure_registry, call)
    }
}
