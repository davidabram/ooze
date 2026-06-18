use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutantStatus {
    Killed,
    Survived,
    Timeout,
    Error,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MutationRunReport {
    pub total: usize,
    pub killed: usize,
    pub survived: usize,
    pub timeout: usize,
    pub error: usize,
    pub outcomes: Vec<MutantOutcome>,
}

impl MutationRunReport {
    pub fn from_outcomes(outcomes: Vec<MutantOutcome>) -> Self {
        let total = outcomes.len();

        let killed = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Killed))
            .count();

        let survived = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Survived))
            .count();

        let timeout = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Timeout))
            .count();

        let error = outcomes
            .iter()
            .filter(|o| matches!(o.status, MutantStatus::Error))
            .count();

        Self {
            total,
            killed,
            survived,
            timeout,
            error,
            outcomes,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MutantOutcome {
    pub candidate: MutationCandidate,
    pub status: MutantStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub diff: String,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppliedMutation {
    pub candidate: MutationCandidate,
    pub workspace_file: PathBuf,
    pub diff: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FunctionSpan {
    pub file: PathBuf,
    pub language: String,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub cyclomatic: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CrapEntry {
    pub file: PathBuf,
    pub language: String,
    pub function: String,
    pub line: usize,
    pub cyclomatic: usize,
    pub coverage: f64,
    pub crap: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FileCoverage {
    pub lines: BTreeMap<u32, u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorName {
    SwapBoolean,
    NegateEquality,
    SwapComparison,
    SwapLogical,
    IntegerZeroOne,
}

impl std::fmt::Display for OperatorName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        f.write_str(&s)
    }
}

pub struct MutationOperator {
    pub name: OperatorName,
    pub query: &'static str,
    pub replacement: fn(&str) -> Option<String>,
    pub description: fn(&str, &str) -> String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MutationCandidate {
    pub id: String,
    pub file: PathBuf,
    pub language: String,
    pub function: String,
    pub operator: OperatorName,
    pub line: usize,
    pub column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub original: String,
    pub replacement: String,
    pub description: String,
}

impl FileCoverage {
    pub fn coverage_in_span(&self, start_line: usize, end_line: usize) -> f64 {
        let start = start_line as u32;
        let end = end_line as u32;

        let executable: Vec<_> = self.lines.range(start..=end).collect();

        if executable.is_empty() {
            return 100.0;
        }

        let covered = executable
            .iter()
            .filter(|(_, hits)| **hits > 0)
            .count();

        covered as f64 / executable.len() as f64 * 100.0
    }
}
