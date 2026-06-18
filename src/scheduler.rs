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

fn actionable_score(entry: Option<&CrapEntry>) -> i32 {
    let Some(e) = entry else {
        return -80;
    };

    let mut score = 0;

    if (15.0..=40.0).contains(&e.crap) {
        score += 100;
    }
    if (5..=15).contains(&e.cyclomatic) {
        score += 50;
    }
    if (60.0..=90.0).contains(&e.coverage) {
        score += 50;
    }
    if e.coverage <= 0.0 {
        score -= 80;
    }
    if e.crap > 80.0 {
        score -= 50;
    }

    score
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
