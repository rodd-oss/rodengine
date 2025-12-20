//! Tick phase handling and scheduling logic

use std::time::Instant;

use crate::api_request::TickPhase;
use crate::Result;

/// Trait for tick phase processing
pub trait TickPhaseProcessor {
    /// Process a specific tick phase
    fn process_tick_phase(&mut self, phase: TickPhase, tick_start: Instant) -> Result<()>;
}

impl TickPhaseProcessor for crate::Runtime {
    /// Process a specific tick phase
    fn process_tick_phase(&mut self, phase: TickPhase, tick_start: Instant) -> Result<()> {
        match phase {
            TickPhase::Api => self.process_api_phase(tick_start),
            TickPhase::Procedures => self.process_procedure_phase(tick_start),
            TickPhase::Persistence => self.process_persistence_phase(),
        }
    }
}
