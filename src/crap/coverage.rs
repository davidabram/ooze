use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::core::FileCoverage;

pub fn parse_lcov(path: &Path) -> Result<HashMap<PathBuf, FileCoverage>> {
    // Branch records are ignored below, but some producers (e.g. coverage.py)
    // emit non-numeric `BRDA` branch fields like "jump to line 6", which the
    // strict `lcov` parser rejects. Strip branch records so they can't abort
    // the parse over data we don't use.
    let contents = fs::read_to_string(path)
        .with_context(|| format!("opening LCOV file {}", path.display()))?;
    let filtered: String = contents
        .lines()
        .filter(|line| {
            !(line.starts_with("BRDA:")
                || line.starts_with("BRF:")
                || line.starts_with("BRH:"))
        })
        .map(|line| format!("{line}\n"))
        .collect();
    let reader = lcov::Reader::new(filtered.as_bytes());

    let mut files: HashMap<PathBuf, FileCoverage> = HashMap::new();
    let mut current_file: Option<PathBuf> = None;

    for record in reader {
        let record = record
            .with_context(|| format!("parsing LCOV record in {}", path.display()))?;

        match record {
            lcov::Record::SourceFile { path: sf_path } => {
                current_file = Some(sf_path.clone());
                files.entry(sf_path).or_default();
            }

            lcov::Record::LineData { line, count, .. } => {
                let Some(file) = current_file.as_ref() else {
                    continue;
                };

                files
                    .entry(file.clone())
                    .or_default()
                    .lines
                    .entry(line)
                    .and_modify(|hits| *hits += count)
                    .or_insert(count);
            }

            lcov::Record::EndOfRecord => {
                current_file = None;
            }

            _ => {}
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_coverage_py_textual_branches() {
        // coverage.py emits non-numeric BRDA branch fields ("jump to line 6")
        // that the strict lcov crate would otherwise reject.
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(
            f,
            "SF:src/yooxn/__main__.py\n\
             DA:5,0\n\
             DA:6,1\n\
             LF:2\n\
             LH:1\n\
             BRDA:5,0,jump to line 6,-\n\
             BRDA:5,0,exit the module,-\n\
             BRF:2\n\
             BRH:0\n\
             end_of_record\n"
        )
        .unwrap();

        let cov = parse_lcov(f.path()).unwrap();
        let file = cov.get(Path::new("src/yooxn/__main__.py")).unwrap();
        assert_eq!(file.lines.get(&5), Some(&0));
        assert_eq!(file.lines.get(&6), Some(&1));
    }
}
