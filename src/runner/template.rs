//! Probe-env value templates.
//!
//! Probe-env values may reference `{worker}` (the worker index) and
//! `{build_cache}` (the active build-cache dir). Previously every consumer
//! re-ran `str::replace` for both placeholders, with subtly different rules
//! scattered across the runner and CLI. A `ProbeEnvTemplate` parses each value
//! once into a flat segment list; consumers then only evaluate against a typed
//! context.
//!
//! Only the two known placeholders are recognized — any other text (including
//! stray braces) is preserved verbatim, matching the original replace-based
//! behavior.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Literal(String),
    Worker,
    BuildCache,
}

/// A `KEY` plus its parsed `VALUE`, ready to evaluate per worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeEnvTemplate {
    key: String,
    segs: Vec<Seg>,
}

/// The values a template is evaluated against. `build_cache` is `None` when no
/// build-cache dir applies; in that case `{build_cache}` is left literal.
#[derive(Debug, Clone, Copy)]
pub struct ProbeEnvCtx<'a> {
    pub worker: usize,
    pub build_cache: Option<&'a Path>,
}

impl ProbeEnvTemplate {
    /// Parse one `KEY` and raw `VALUE`. Infallible: unrecognized braces are kept
    /// as literal text.
    pub fn parse(key: String, value: &str) -> Self {
        const WORKER: &str = "{worker}";
        const BUILD_CACHE: &str = "{build_cache}";

        let mut segs = Vec::new();
        let mut literal = String::new();
        let mut rest = value;
        while !rest.is_empty() {
            if let Some(tail) = rest.strip_prefix(WORKER) {
                flush(&mut literal, &mut segs);
                segs.push(Seg::Worker);
                rest = tail;
            } else if let Some(tail) = rest.strip_prefix(BUILD_CACHE) {
                flush(&mut literal, &mut segs);
                segs.push(Seg::BuildCache);
                rest = tail;
            } else {
                // Consume one char of literal text and continue scanning.
                let mut chars = rest.chars();
                let c = chars.next().expect("rest is non-empty");
                literal.push(c);
                rest = chars.as_str();
            }
        }
        flush(&mut literal, &mut segs);

        Self { key, segs }
    }

    /// The env var name this template sets.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Whether the value references `{worker}` (so it expands per worker index).
    pub fn references_worker(&self) -> bool {
        self.segs.contains(&Seg::Worker)
    }

    /// Evaluate the value against `ctx`.
    pub fn eval(&self, ctx: ProbeEnvCtx<'_>) -> String {
        let mut out = String::new();
        for seg in &self.segs {
            match seg {
                Seg::Literal(s) => out.push_str(s),
                Seg::Worker => out.push_str(&ctx.worker.to_string()),
                Seg::BuildCache => match ctx.build_cache {
                    Some(dir) => out.push_str(&dir.to_string_lossy()),
                    // No build cache in scope: preserve the placeholder, as the
                    // original replace-based code did.
                    None => out.push_str("{build_cache}"),
                },
            }
        }
        out
    }

    /// Evaluate to a `(key, value)` pair for the env APIs.
    pub fn eval_pair(&self, ctx: ProbeEnvCtx<'_>) -> (String, String) {
        (self.key.clone(), self.eval(ctx))
    }
}

/// Evaluate a whole template list against one context.
pub fn eval_all(templates: &[ProbeEnvTemplate], ctx: ProbeEnvCtx<'_>) -> Vec<(String, String)> {
    templates.iter().map(|t| t.eval_pair(ctx)).collect()
}

fn flush(literal: &mut String, segs: &mut Vec<Seg>) {
    if !literal.is_empty() {
        segs.push(Seg::Literal(std::mem::take(literal)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx(worker: usize, build_cache: Option<&Path>) -> ProbeEnvCtx<'_> {
        ProbeEnvCtx { worker, build_cache }
    }

    #[test]
    fn plain_literal_passes_through() {
        let t = ProbeEnvTemplate::parse("K".into(), "plain/value");
        assert!(!t.references_worker());
        assert_eq!(t.eval(ctx(3, None)), "plain/value");
    }

    #[test]
    fn worker_is_substituted() {
        let t = ProbeEnvTemplate::parse("K".into(), "dir/{worker}");
        assert!(t.references_worker());
        assert_eq!(t.eval(ctx(2, None)), "dir/2");
    }

    #[test]
    fn build_cache_substituted_when_present_else_literal() {
        let t = ProbeEnvTemplate::parse("K".into(), "x={build_cache}");
        assert_eq!(t.eval(ctx(0, Some(Path::new("/c")))), "x=/c");
        // Lenient: no cache in scope leaves the placeholder, matching old behavior.
        assert_eq!(t.eval(ctx(0, None)), "x={build_cache}");
        assert!(!t.references_worker());
    }

    #[test]
    fn mixed_placeholders_and_adjacent_tokens() {
        let t = ProbeEnvTemplate::parse("K".into(), "{worker}-{build_cache}/sub");
        assert_eq!(
            t.eval(ctx(7, Some(Path::new("/cache")))),
            "7-/cache/sub"
        );
    }

    #[test]
    fn unknown_braces_are_literal() {
        // A value with literal braces (e.g. JSON-ish) must be untouched.
        let t = ProbeEnvTemplate::parse("K".into(), "{foo}{worker}");
        assert_eq!(t.eval(ctx(1, None)), "{foo}1");
    }

    #[test]
    fn eval_pair_carries_key() {
        let t = ProbeEnvTemplate::parse("CACHE".into(), "{worker}");
        assert_eq!(
            t.eval_pair(ctx(4, None)),
            ("CACHE".to_string(), "4".to_string())
        );
    }
}
