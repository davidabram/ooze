//! Go coverprofile parser (`coverage.out`, `cover.out`).
//!
//! Produced by `go test -coverprofile=coverage.out ./...` (and
//! `go tool covdata textfmt`). The first line is a `mode:` header; every other
//! line describes a block:
//!
//! ```text
//! github.com/me/app/foo.go:10.13,12.2 1 1
//! ```
//!
//! i.e. `file:startLine.startCol,endLine.endCol numStmts count`. Each line in
//! the block range is marked executable, hit when `count > 0`.

use std::path::Path;

use anyhow::{Context, Result};

use super::CoverageMap;

pub fn parse(path: &Path) -> Result<CoverageMap> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("opening Go coverprofile {}", path.display()))?;
    Ok(parse_str(&contents))
}

fn parse_str(contents: &str) -> CoverageMap {
    let mut files = CoverageMap::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("mode:") {
            continue;
        }

        let Some((file, start, end, count)) = parse_block(line) else {
            continue;
        };

        let file_cov = files.entry(file.into()).or_default();
        for n in start..=end {
            let entry = file_cov.lines.entry(n).or_insert(0);
            *entry = (*entry).max(count);
        }
    }

    files
}

/// Parse one block line into `(file, start_line, end_line, count)`.
fn parse_block(line: &str) -> Option<(&str, u32, u32, u64)> {
    // `file:startLine.startCol,endLine.endCol numStmts count`
    let mut fields = line.split_whitespace();
    let range = fields.next()?;
    let count = fields.last()?.parse::<u64>().ok()?;

    let (file, span) = range.rsplit_once(':')?;
    let (start, end) = span.split_once(',')?;
    let start_line = start.split('.').next()?.parse::<u32>().ok()?;
    let end_line = end.split('.').next()?.parse::<u32>().ok()?;

    Some((file, start_line, end_line, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_block_ranges_and_keeps_max_hits() {
        let profile = "mode: set\n\
            github.com/me/app/foo.go:10.13,12.2 2 1\n\
            github.com/me/app/foo.go:15.2,18.3 3 0\n";

        let cov = parse_str(profile);
        let file = cov.get(Path::new("github.com/me/app/foo.go")).unwrap();
        // Covered block 10..=12.
        assert_eq!(file.lines.get(&10), Some(&1));
        assert_eq!(file.lines.get(&12), Some(&1));
        // Uncovered block 15..=18.
        assert_eq!(file.lines.get(&15), Some(&0));
        assert_eq!(file.lines.get(&18), Some(&0));
    }
}
