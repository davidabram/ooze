//! Raw probe process execution: spawn the probe in a workspace, drain its
//! output, enforce the timeout, and classify the outcome.

use crate::core::{AppliedMutation, MutantOutcome, MutantStatus};
use crate::probe::ProbeCommand;
use anyhow::{Context, Result};
use std::io::Read as _;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Result of running a child process to completion (or timeout). `status` is
/// `None` exactly when `timed_out` is true.
pub(super) struct ProcessRun {
    pub(super) status: Option<ExitStatus>,
    pub(super) timed_out: bool,
    pub(super) duration_ms: u128,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

/// Wait for a spawned child while concurrently draining its stdout/stderr.
///
/// The pipes are read on separate threads so a probe that writes more than a
/// pipe buffer's worth (~64KB on Linux) before exiting can't block on its own
/// write and be misclassified as a timeout. `started` is the instant the child
/// was spawned, used for the timeout budget and reported duration.
pub(super) fn wait_with_drained_output(
    mut child: Child,
    timeout: Option<Duration>,
    started: Instant,
) -> Result<ProcessRun> {
    // Take the pipe handles and drain each on its own thread. Both threads see
    // EOF once the child exits (or is killed), so they always finish.
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = out.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = err.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });

    let mut timed_out = false;
    let status = loop {
        if let Some(s) = child.try_wait().context("polling probe child")? {
            break Some(s);
        }
        if let Some(limit) = timeout
            && started.elapsed() >= limit
        {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let duration_ms = started.elapsed().as_millis();
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();

    Ok(ProcessRun {
        status,
        timed_out,
        duration_ms,
        stdout,
        stderr,
    })
}

pub fn run_probe(
    workspace_path: &Path,
    applied: AppliedMutation,
    probe: &ProbeCommand,
    timeout: Option<Duration>,
    extra_envs: &[(String, String)],
) -> Result<MutantOutcome> {
    let started = Instant::now();

    let mut cmd = Command::new(probe.program());
    cmd.args(probe.args())
        .current_dir(workspace_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("spawning probe command {probe:?}"))?;

    let ProcessRun {
        status,
        timed_out,
        duration_ms,
        stdout,
        stderr,
    } = wait_with_drained_output(child, timeout, started)?;

    let (mutant_status, exit_code) = if timed_out {
        (MutantStatus::Timeout, None)
    } else {
        let s = status.expect("status set when not timed out");
        let code = s.code();
        let mutant_status = if s.success() {
            MutantStatus::Survived
        } else {
            MutantStatus::Killed
        };
        (mutant_status, code)
    };

    Ok(MutantOutcome {
        candidate: applied.candidate,
        status: mutant_status,
        exit_code,
        duration_ms,
        diff: applied.diff,
        stdout,
        stderr,
    })
}
