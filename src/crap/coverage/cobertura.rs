//! Cobertura XML parser (`coverage.xml`, `cobertura.xml`).
//!
//! Produced by coverage.py (`coverage xml`), JS/TS tooling, gcovr, and many CI
//! systems. Each `<class filename="...">` carries `<line number="N" hits="H"/>`
//! entries, which map directly onto ooze's `file -> line -> hits` model.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use super::CoverageMap;

pub fn parse(path: &Path) -> Result<CoverageMap> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("opening Cobertura file {}", path.display()))?;
    parse_str(&contents)
        .with_context(|| format!("parsing Cobertura file {}", path.display()))
}

fn parse_str(xml: &str) -> Result<CoverageMap> {
    let mut reader = Reader::from_str(xml);
    let mut files = CoverageMap::new();
    let mut current_file: Option<PathBuf> = None;

    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                b"class" => {
                    if let Some(filename) = attr(&e, b"filename") {
                        let file = PathBuf::from(filename);
                        files.entry(file.clone()).or_default();
                        current_file = Some(file);
                    }
                }
                b"line" => {
                    let Some(file) = current_file.as_ref() else {
                        continue;
                    };
                    if let (Some(number), Some(hits)) = (
                        attr(&e, b"number").and_then(|s| s.parse::<u32>().ok()),
                        attr(&e, b"hits").and_then(|s| s.parse::<u64>().ok()),
                    ) {
                        let entry = files
                            .entry(file.clone())
                            .or_default()
                            .lines
                            .entry(number)
                            .or_insert(0);
                        *entry = (*entry).max(hits);
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(files)
}

/// Read an attribute as an owned, unescaped string.
fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| {
            #[allow(deprecated)]
            a.unescape_value().ok().map(std::borrow::Cow::into_owned)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines_per_file() {
        let xml = r#"<?xml version="1.0"?>
<coverage>
  <packages>
    <package name="src">
      <classes>
        <class filename="src/foo.py">
          <lines>
            <line number="10" hits="1"/>
            <line number="11" hits="0"/>
          </lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>"#;

        let cov = parse_str(xml).unwrap();
        let file = cov.get(Path::new("src/foo.py")).unwrap();
        assert_eq!(file.lines.get(&10), Some(&1));
        assert_eq!(file.lines.get(&11), Some(&0));
    }
}
