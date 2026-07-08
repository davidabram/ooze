//! Execution: how probes are run across mutants.
//!
//! `crate::workspace` decides where a mutant is applied; this module owns
//! everything about running the probe command — child process handling and
//! timeouts, the sequential/parallel batch drivers, the baseline preflight
//! check, and build-cache warmup.

mod batch;
mod preflight;
mod process;
pub mod template;
mod warmup;

pub use batch::{BatchConfig, ProgressEvent, run_mutants_parallel};
pub use preflight::preflight;
pub use process::run_probe;
pub use template::{ProbeEnvCtx, ProbeEnvTemplate};
pub use warmup::{warmup, warmup_workers};
