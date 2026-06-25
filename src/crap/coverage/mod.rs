//! Coverage report parsers.
//!
//! Every parser normalizes its input into the same shape ooze already wants:
//! `file -> line -> hit count`. Block/range formats (Go) expand each covered
//! range into individual executable lines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::core::FileCoverage;

mod cobertura;
mod go_cover;
mod jacoco;
mod lcov;

pub use lcov::parse_lcov;

/// A parsed coverage report keyed by source file.
pub type CoverageMap = HashMap<PathBuf, FileCoverage>;

/// Supported coverage report formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageFormat {
    Lcov,
    Cobertura,
    Jacoco,
    GoCover,
}

impl CoverageFormat {
    /// Parse a `--coverage` format keyword (the part before `:`).
    fn from_keyword(keyword: &str) -> Option<Self> {
        match keyword {
            "lcov" => Some(Self::Lcov),
            "cobertura" => Some(Self::Cobertura),
            "jacoco" => Some(Self::Jacoco),
            "go-cover" | "gocover" | "go" => Some(Self::GoCover),
            _ => None,
        }
    }

    fn parse(self, path: &Path) -> Result<CoverageMap> {
        match self {
            Self::Lcov => lcov::parse_lcov(path),
            Self::Cobertura => cobertura::parse(path),
            Self::Jacoco => jacoco::parse(path),
            Self::GoCover => go_cover::parse(path),
        }
    }
}

/// Load coverage from a `--coverage` spec.
///
/// Accepts an explicit `format:path` (e.g. `cobertura:coverage.xml`) or a bare
/// path that is auto-detected from its name and contents.
pub fn load(spec: &str) -> Result<CoverageMap> {
    // Only treat the prefix as a format if it's a keyword we recognize;
    // this keeps Windows drive paths (`C:\...`) and plain paths working.
    if let Some((keyword, rest)) = spec.split_once(':')
        && let Some(format) = CoverageFormat::from_keyword(keyword)
    {
        return format.parse(Path::new(rest));
    }

    let path = Path::new(spec);
    let format = detect(path)?;
    format.parse(path)
}

/// Load and merge several `--coverage` specs into a single map.
///
/// Repos with split test suites (e.g. a JS frontend and a JVM backend) produce
/// one report per suite; merging lets a single ooze run see all of them.
pub fn load_all(specs: &[String]) -> Result<CoverageMap> {
    let mut acc = CoverageMap::new();
    for spec in specs {
        merge_into(&mut acc, load(spec)?);
    }
    Ok(acc)
}

/// Merge `from` into `acc`, summing hit counts for the same file and line.
fn merge_into(acc: &mut CoverageMap, from: CoverageMap) {
    for (file, cov) in from {
        let entry = acc.entry(file).or_default();
        for (line, hits) in cov.lines {
            entry
                .lines
                .entry(line)
                .and_modify(|h| *h += hits)
                .or_insert(hits);
        }
    }
}

/// Guess the coverage format of a bare path from its name and a peek at its
/// contents.
fn detect(path: &Path) -> Result<CoverageFormat> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    if has_extension(path, "info") {
        return Ok(CoverageFormat::Lcov);
    }

    // Read a small prefix to sniff content-based formats.
    let head = read_head(path).unwrap_or_default();

    if name == "coverage.out" || name == "cover.out" || head.trim_start().starts_with("mode:") {
        return Ok(CoverageFormat::GoCover);
    }

    if has_extension(path, "xml") || head.contains('<') {
        // JaCoCo's root element is <report>; Cobertura's is <coverage>.
        if head.contains("<report") {
            return Ok(CoverageFormat::Jacoco);
        }
        if head.contains("<coverage") {
            return Ok(CoverageFormat::Cobertura);
        }
    }

    bail!(
        "could not detect coverage format for {}; pass an explicit format, e.g. --coverage cobertura:{}",
        path.display(),
        path.display()
    )
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn read_head(path: &Path) -> Result<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 4096];
    let read = file.read(&mut buf)?;
    buf.truncate(read);
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_sums_hits_for_same_file_and_line() {
        let mut acc = CoverageMap::new();
        acc.entry(PathBuf::from("src/foo.rs"))
            .or_default()
            .lines
            .extend([(1, 1), (2, 0)]);

        let mut from = CoverageMap::new();
        from.entry(PathBuf::from("src/foo.rs"))
            .or_default()
            .lines
            .extend([(2, 3), (3, 1)]);

        merge_into(&mut acc, from);

        let foo = acc.get(Path::new("src/foo.rs")).unwrap();
        assert_eq!(foo.lines.get(&1), Some(&1));
        assert_eq!(foo.lines.get(&2), Some(&3)); // 0 + 3
        assert_eq!(foo.lines.get(&3), Some(&1));
    }

    #[test]
    fn keyword_prefix_selects_format() {
        assert_eq!(
            CoverageFormat::from_keyword("cobertura"),
            Some(CoverageFormat::Cobertura)
        );
        assert_eq!(
            CoverageFormat::from_keyword("go-cover"),
            Some(CoverageFormat::GoCover)
        );
        assert_eq!(CoverageFormat::from_keyword("nope"), None);
    }
}
