//! `JaCoCo` XML parser (`jacoco.xml`).
//!
//! The de-facto coverage report for JVM languages (Java, Kotlin, Scala,
//! Groovy). Line coverage lives under `<package name="..."><sourcefile
//! name="..."><line nr="N" ci="C" .../>`. The full source path is the package
//! name joined with the sourcefile name; a line counts as hit when its covered
//! instructions (`ci`) is greater than zero.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use super::CoverageMap;

pub fn parse(path: &Path) -> Result<CoverageMap> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("opening JaCoCo file {}", path.display()))?;
    parse_str(&contents).with_context(|| format!("parsing JaCoCo file {}", path.display()))
}

fn parse_str(xml: &str) -> Result<CoverageMap> {
    let mut reader = Reader::from_str(xml);
    let mut files = CoverageMap::new();
    let mut current_package = String::new();
    let mut current_file: Option<PathBuf> = None;

    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                b"package" => {
                    current_package = attr(&e, b"name").unwrap_or_default();
                }
                b"sourcefile" => {
                    current_file = attr(&e, b"name").map(|name| {
                        if current_package.is_empty() {
                            PathBuf::from(name)
                        } else {
                            PathBuf::from(format!("{current_package}/{name}"))
                        }
                    });
                    if let Some(file) = current_file.as_ref() {
                        files.entry(file.clone()).or_default();
                    }
                }
                b"line" => {
                    let Some(file) = current_file.as_ref() else {
                        continue;
                    };
                    if let Some(nr) = attr(&e, b"nr").and_then(|s| s.parse::<u32>().ok()) {
                        let ci = attr(&e, b"ci")
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0);
                        files
                            .entry(file.clone())
                            .or_default()
                            .lines
                            .insert(nr, u64::from(ci > 0));
                    }
                }
                _ => {}
            },
            Event::End(e) if e.local_name().as_ref() == b"sourcefile" => {
                current_file = None;
            }
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
    fn joins_package_and_sourcefile() {
        let xml = r#"<?xml version="1.0"?>
<report name="app">
  <package name="com/example">
    <sourcefile name="Foo.java">
      <line nr="10" mi="0" ci="3" mb="0" cb="0"/>
      <line nr="11" mi="2" ci="0" mb="0" cb="0"/>
    </sourcefile>
  </package>
</report>"#;

        let cov = parse_str(xml).unwrap();
        let file = cov.get(Path::new("com/example/Foo.java")).unwrap();
        assert_eq!(file.lines.get(&10), Some(&1));
        assert_eq!(file.lines.get(&11), Some(&0));
    }
}
