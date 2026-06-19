use crate::core::{CrapEntry, MutationCandidate};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum MutationStrategy {
    Discovery,
    Actionable,
    HighestCrap,
}

pub fn order(
    strategy: MutationStrategy,
    candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
) -> Vec<MutationCandidate> {
    match strategy {
        MutationStrategy::Discovery => candidates,
        MutationStrategy::Actionable => rank_actionable(candidates, crap_entries),
        MutationStrategy::HighestCrap => rank_highest_crap(candidates, crap_entries),
    }
}

type CrapIndex<'a> = HashMap<(PathBuf, String), &'a CrapEntry>;

fn index_crap(crap_entries: &[CrapEntry]) -> CrapIndex<'_> {
    crap_entries
        .iter()
        .map(|e| ((e.file.clone(), e.function.clone()), e))
        .collect()
}

fn lookup<'a>(index: &'a CrapIndex<'a>, c: &MutationCandidate) -> Option<&'a CrapEntry> {
    index
        .get(&(c.file.clone(), c.function.clone()))
        .copied()
}

fn actionable_score_with_reasons(entry: Option<&CrapEntry>) -> (i32, Vec<String>) {
    let Some(e) = entry else {
        return (-80, vec!["no CRAP entry for function".to_string()]);
    };

    let mut score = 0;
    let mut reasons = Vec::new();

    if (15.0..=40.0).contains(&e.crap) {
        score += 100;
        reasons.push(format!("CRAP {:.1} in actionable range [15,40]", e.crap));
    }
    if (5..=15).contains(&e.cyclomatic) {
        score += 50;
        reasons.push(format!("cyclomatic {} in actionable range [5,15]", e.cyclomatic));
    }
    if (60.0..=90.0).contains(&e.coverage) {
        score += 50;
        reasons.push(format!(
            "coverage {:.1}% in actionable range [60,90]",
            e.coverage
        ));
    }
    if e.coverage <= 0.0 {
        score -= 80;
        reasons.push("uncovered function".to_string());
    }
    if e.crap > 80.0 {
        score -= 50;
        reasons.push(format!("CRAP {:.1} above actionable ceiling 80", e.crap));
    }

    (score, reasons)
}

fn actionable_score(entry: Option<&CrapEntry>) -> i32 {
    actionable_score_with_reasons(entry).0
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SelectionExplanation {
    pub selection_score: i32,
    pub selection_reasons: Vec<String>,
    pub crap: Option<f64>,
    pub cyclomatic: Option<usize>,
    pub coverage: Option<f64>,
}

pub fn explain(
    strategy: MutationStrategy,
    candidate: &MutationCandidate,
    crap_entries: &[CrapEntry],
) -> SelectionExplanation {
    let index = index_crap(crap_entries);
    let entry = lookup(&index, candidate);

    let (score, mut reasons) = match strategy {
        MutationStrategy::Discovery => (0, vec!["discovery order (no scoring)".to_string()]),
        MutationStrategy::Actionable => actionable_score_with_reasons(entry),
        MutationStrategy::HighestCrap => {
            let s = entry
                .map(|e| e.crap.round() as i32)
                .unwrap_or(i32::MIN);
            let r = match entry {
                Some(e) => vec![format!("CRAP {:.1}", e.crap)],
                None => vec!["no CRAP entry for function".to_string()],
            };
            (s, r)
        }
    };

    if reasons.is_empty() {
        reasons.push("no scoring rules matched".to_string());
    }

    SelectionExplanation {
        selection_score: score,
        selection_reasons: reasons,
        crap: entry.map(|e| e.crap),
        cyclomatic: entry.map(|e| e.cyclomatic),
        coverage: entry.map(|e| e.coverage),
    }
}

fn rank_actionable(
    mut candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
) -> Vec<MutationCandidate> {
    let index = index_crap(crap_entries);

    candidates.sort_by(|a, b| {
        let sa = actionable_score(lookup(&index, a));
        let sb = actionable_score(lookup(&index, b));
        sb.cmp(&sa).then_with(|| a.id.cmp(&b.id))
    });

    candidates
}

fn rank_highest_crap(
    mut candidates: Vec<MutationCandidate>,
    crap_entries: &[CrapEntry],
) -> Vec<MutationCandidate> {
    let index = index_crap(crap_entries);

    candidates.sort_by(|a, b| {
        let ca = lookup(&index, a).map(|e| e.crap).unwrap_or(f64::NEG_INFINITY);
        let cb = lookup(&index, b).map(|e| e.crap).unwrap_or(f64::NEG_INFINITY);
        cb.partial_cmp(&ca)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    candidates
}
