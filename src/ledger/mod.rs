//! Persistent run ledger: one directory per `test-mutants` run under the
//! runs dir, holding `metadata.json`, `plan.json`, `events.jsonl`, and
//! `report.json`. Written for every report format so a run can be inspected
//! after the command exits; the foundation for future `runs list` / `--resume`.

use crate::execution::ExecutionEvent;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Run-level information, written to `metadata.json` when the ledger is created.
#[derive(Debug, serde::Serialize)]
pub(crate) struct RunMetadata {
    pub run_id: String,
    pub started_at: String,
    pub repo_root: PathBuf,
    pub jobs: usize,
    pub format: String,
    pub probe: Vec<String>,
    pub strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    pub workspace_backend: String,
}

pub(crate) struct RunLedger {
    run_id: String,
    dir: PathBuf,
    /// `events.jsonl`, opened once at creation. Rayon workers append
    /// concurrently through the event sink, so writes go through a mutex.
    events: Mutex<File>,
}

impl RunLedger {
    /// Create `<runs_dir>/<run-id>/`, write `metadata.json`, and open
    /// `events.jsonl` for appending.
    pub(crate) fn create(runs_dir: &Path, metadata: RunMetadata) -> Result<Self> {
        let dir = runs_dir.join(&metadata.run_id);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating run ledger dir {}", dir.display()))?;
        write_pretty_json(&dir.join("metadata.json"), &metadata)?;
        let events_path = dir.join("events.jsonl");
        let events = File::create(&events_path)
            .with_context(|| format!("creating {}", events_path.display()))?;
        Ok(Self {
            run_id: metadata.run_id,
            dir,
            events: Mutex::new(events),
        })
    }

    #[allow(dead_code)] // future `runs list` / `--resume` surface
    pub(crate) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn write_plan<T: serde::Serialize>(&self, plan: &T) -> Result<()> {
        write_pretty_json(&self.dir.join("plan.json"), plan)
    }

    /// Append one compact JSON line to `events.jsonl`. Safe to call from
    /// parallel workers.
    pub(crate) fn append_event(&self, event: &ExecutionEvent) -> Result<()> {
        let line = serde_json::to_string(event).context("serializing execution event")?;
        let mut file = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        writeln!(file, "{line}").context("appending to events.jsonl")?;
        Ok(())
    }

    pub(crate) fn write_report<T: serde::Serialize>(&self, report: &T) -> Result<()> {
        write_pretty_json(&self.dir.join("report.json"), report)
    }
}

fn write_pretty_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value)
        .with_context(|| format!("serializing {}", path.display()))?;
    text.push('\n');
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Filesystem-safe run id: `run-YYYYMMDD-HHMMSS-<pid>`. Timestamp plus pid is
/// unique enough for one run per process without pulling in a UUID dependency.
pub(crate) fn new_run_id() -> String {
    let (year, month, day, hour, minute, second) = utc_now_parts();
    format!(
        "run-{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}-{}",
        std::process::id()
    )
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn utc_timestamp() -> String {
    let (year, month, day, hour, minute, second) = utc_now_parts();
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn utc_now_parts() -> (i64, u32, u32, u32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_from_unix(secs)
}

// Days-to-civil conversion (Howard Hinnant's `civil_from_days`), so run ids
// get readable UTC dates without a chrono/time dependency. All intermediate
// values are far from the integer bounds for any realistic clock reading.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn civil_from_unix(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = (rem / 3600) as u32;
    let minute = (rem % 3600 / 60) as u32;
    let second = (rem % 60) as u32;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, hour, minute, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    // One lifecycle drift guard: create → metadata, events append as JSONL,
    // plan/report land as pretty JSON.
    #[test]
    fn ledger_lifecycle_writes_all_artifacts() {
        let tmp = tempfile::tempdir().unwrap();
        let ledger = RunLedger::create(
            tmp.path(),
            RunMetadata {
                run_id: "run-20260709-103012-42".into(),
                started_at: "2026-07-09T10:30:12Z".into(),
                repo_root: PathBuf::from("/repo"),
                jobs: 2,
                format: "jsonl".into(),
                probe: vec!["cargo".into(), "test".into()],
                strategy: "discovery".into(),
                limit: Some(5),
                seed: None,
                workspace_backend: "copy".into(),
            },
        )
        .unwrap();
        assert_eq!(ledger.dir(), tmp.path().join("run-20260709-103012-42"));

        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(ledger.dir().join("metadata.json")).unwrap())
                .unwrap();
        assert_eq!(meta["run_id"], "run-20260709-103012-42");
        assert_eq!(meta["probe"], serde_json::json!(["cargo", "test"]));

        ledger
            .append_event(&ExecutionEvent::RunStarted { total: 1, jobs: 2 })
            .unwrap();
        ledger
            .append_event(&ExecutionEvent::RunFinished {
                total: 1,
                killed: 1,
                survived: 0,
                timeout: 0,
                error: 0,
            })
            .unwrap();
        let events = std::fs::read_to_string(ledger.dir().join("events.jsonl")).unwrap();
        let lines: Vec<serde_json::Value> = events
            .lines()
            .map(|l| serde_json::from_str(l).expect("each ledger event line is JSON"))
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["event"], "run_started");
        assert_eq!(lines[1]["event"], "run_finished");

        ledger.write_plan(&serde_json::json!({"selected": 1})).unwrap();
        ledger
            .write_report(&serde_json::json!({"total": 1}))
            .unwrap();
        let report = std::fs::read_to_string(ledger.dir().join("report.json")).unwrap();
        assert!(report.contains('\n'), "report.json is pretty-printed");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&report).unwrap()["total"],
            1
        );
        assert!(ledger.dir().join("plan.json").exists());
    }

    #[test]
    fn civil_from_unix_converts_known_timestamp() {
        // 2026-07-09T10:30:12Z
        assert_eq!(civil_from_unix(1_783_593_012), (2026, 7, 9, 10, 30, 12));
    }
}
