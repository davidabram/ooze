use std::collections::BTreeMap;
use std::path::PathBuf;

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
    pub coverage: Option<f64>,
    pub crap: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FileCoverage {
    pub lines: BTreeMap<u32, u64>,
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
