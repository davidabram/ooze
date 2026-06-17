use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::core::FileCoverage;

pub fn parse_lcov(path: &Path) -> Result<HashMap<PathBuf, FileCoverage>> {
    let reader = lcov::Reader::open_file(path)
        .with_context(|| format!("opening LCOV file {}", path.display()))?;

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
