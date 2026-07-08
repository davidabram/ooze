//! Baseline probe run on unmodified code: mutation results are only valid if
//! the probe passes before any mutant is applied.

use super::process::{ProcessRun, wait_with_drained_output};
use crate::probe::ProbeCommand;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreflightOutcome {
    pub success: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub stdout: String,
    pub stderr: String,
}

pub fn preflight(
    repo_root: &Path,
    probe: &ProbeCommand,
    timeout: Option<Duration>,
    extra_envs: &[(String, String)],
) -> Result<PreflightOutcome> {
    let started = Instant::now();

    let mut cmd = Command::new(probe.program());
    cmd.args(probe.args())
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (k, v) in extra_envs {
        cmd.env(k, v);
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("spawning preflight probe {probe:?}"))?;

    let ProcessRun {
        status,
        timed_out,
        duration_ms,
        stdout,
        stderr,
    } = wait_with_drained_output(child, timeout, started)?;

    let (success, exit_code) = if timed_out {
        (false, None)
    } else {
        let s = status.expect("status set when not timed out");
        (s.success(), s.code())
    };

    Ok(PreflightOutcome {
        success,
        timed_out,
        exit_code,
        duration_ms,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A probe that writes far more than a pipe buffer (~64KB) before exiting
    /// must be drained concurrently — otherwise it blocks on its own write and
    /// is misclassified as a timeout. 200KB exceeds the buffer several times
    /// over, so this fails (as a timeout) if the output isn't drained.
    #[test]
    #[cfg(unix)]
    fn large_probe_output_is_drained_without_false_timeout() {
        let probe = ProbeCommand::from_static(&["sh", "-c", "yes | head -c 200000"]);
        let outcome = preflight(Path::new("."), &probe, Some(Duration::from_secs(30)), &[])
            .expect("preflight should run");

        assert!(
            !outcome.timed_out,
            "200KB of output was misread as a timeout"
        );
        assert!(outcome.success, "probe exited 0");
        assert_eq!(outcome.stdout.len(), 200_000, "full stdout captured");
    }
}
