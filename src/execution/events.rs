//! Streamable execution events: what a mutation run already does, exposed as
//! a compact event stream for agent-oriented output (`--format jsonl`).
//!
//! Events deliberately omit heavy per-mutant payloads (diff, probe
//! stdout/stderr); consumers that need those read the full report instead.

use crate::core::{MutantOutcome, MutantStatus, MutationRunReport, OperatorName};
use std::path::PathBuf;

/// One line in the event stream. Serialized with an `event` tag so consumers
/// can dispatch on `{"event":"run_started",...}` etc.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ExecutionEvent {
    RunStarted {
        total: usize,
        jobs: usize,
    },
    MutantFinished {
        completed: usize,
        total: usize,
        id: String,
        status: MutantStatus,
        duration_ms: u128,
        exit_code: Option<i32>,
        operator: OperatorName,
        file: PathBuf,
        line: usize,
        column: usize,
    },
    RunFinished {
        total: usize,
        killed: usize,
        survived: usize,
        timeout: usize,
        error: usize,
    },
}

impl ExecutionEvent {
    pub fn mutant_finished(completed: usize, total: usize, outcome: &MutantOutcome) -> Self {
        ExecutionEvent::MutantFinished {
            completed,
            total,
            id: outcome.candidate.id.clone(),
            status: outcome.status.clone(),
            duration_ms: outcome.duration_ms,
            exit_code: outcome.exit_code,
            operator: outcome.candidate.operator,
            file: outcome.candidate.file.clone(),
            line: outcome.candidate.line,
            column: outcome.candidate.column,
        }
    }

    pub fn run_finished(report: &MutationRunReport) -> Self {
        ExecutionEvent::RunFinished {
            total: report.total,
            killed: report.killed,
            survived: report.survived,
            timeout: report.timeout,
            error: report.error,
        }
    }
}

/// Callback invoked as execution progresses. Must be `Sync`: parallel workers
/// call it concurrently from the rayon pool.
pub type EventSink<'a> = &'a (dyn Fn(ExecutionEvent) + Sync);
