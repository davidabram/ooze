use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::{
    CrapEntry, MutantOutcome, MutantStatus, MutationCandidate, MutationRunReport, OperatorName,
};
use crate::source_path::FileKey;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentTask {
    pub id: String,
    pub file: PathBuf,
    pub function: String,
    pub line: usize,
    /// 1-based column of the mutation site, matching editor/SARIF convention.
    pub column: usize,
    pub operator: OperatorName,
    pub mutation: String,
    pub focus: String,
    pub operator_hint: String,
    pub prompt: String,
    pub cyclomatic: Option<usize>,
    pub coverage: Option<f64>,
    pub crap: Option<f64>,
    pub priority_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_context: Option<SourceContext>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentTaskReport {
    pub tasks: Vec<AgentTask>,
}

/// Borrowed agent tasks paired with the count of duplicates they represent.
type TaskRefs<'a> = Vec<(&'a AgentTask, usize)>;
/// Tasks grouped by file, then by function.
type TasksByFile<'a> =
    std::collections::BTreeMap<String, std::collections::BTreeMap<String, TaskRefs<'a>>>;

pub fn agent_tasks(report: &EnrichedRunReport) -> AgentTaskReport {
    // Join survived outcomes back to their function summary by source identity,
    // so a difference in path spelling never drops an enrichment.
    let func_index: HashMap<(FileKey, &String), &FunctionMutationSummary> = report
        .functions
        .iter()
        .map(|f| ((FileKey::resolve(&f.file), &f.function), f))
        .collect();

    let mut tasks = Vec::new();
    for o in &report.outcomes {
        if !matches!(o.outcome.status, MutantStatus::Survived) {
            continue;
        }
        let Some(suggestion) = &o.test_suggestion else {
            continue;
        };
        let func_info = func_index
            .get(&(
                FileKey::resolve(&o.outcome.candidate.file),
                &o.outcome.candidate.function,
            ))
            .copied();
        tasks.push(AgentTask {
            id: format!("test-task-{:03}", tasks.len() + 1),
            file: o.outcome.candidate.file.clone(),
            function: o.outcome.candidate.function.clone(),
            line: o.outcome.candidate.line,
            column: o.outcome.candidate.column + 1,
            operator: o.outcome.candidate.operator,
            mutation: format!(
                "{} -> {}",
                o.outcome.candidate.original, o.outcome.candidate.replacement
            ),
            focus: suggestion.focus.clone(),
            operator_hint: suggestion.operator_hint.clone(),
            prompt: suggestion.prompt.clone(),
            cyclomatic: func_info.and_then(|f| f.cyclomatic),
            coverage: func_info.and_then(|f| f.coverage),
            crap: func_info.and_then(|f| f.crap),
            priority_score: func_info.map_or(0.0, |f| f.priority_score),
            source_context: o.source_context.clone(),
        });
    }
    AgentTaskReport { tasks }
}

/// Report verbosity level. Governs which heavy fields (diffs, probe output,
/// source context) and which outcomes land in the serialized report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lower")]
pub enum ReportDetail {
    /// Smallest: survivors only, no diffs or probe output.
    Compact,
    /// Diffs kept, probe stdout/stderr dropped, all outcomes.
    Normal,
    /// Everything: diffs, stdout, stderr, source context, all outcomes.
    Full,
}

/// Resolved set of report-size controls. Built from a `ReportDetail` baseline,
/// then overridden by individual `--no-*` / `--only-survivors` flags.
#[derive(Debug, Clone, Copy)]
pub struct ReportOptions {
    pub include_diff: bool,
    pub include_stdout: bool,
    pub include_stderr: bool,
    pub include_source_context: bool,
    pub only_survivors: bool,
}

impl ReportOptions {
    pub fn from_detail(detail: ReportDetail) -> Self {
        match detail {
            ReportDetail::Full => ReportOptions {
                include_diff: true,
                include_stdout: true,
                include_stderr: true,
                include_source_context: true,
                only_survivors: false,
            },
            ReportDetail::Normal => ReportOptions {
                include_diff: true,
                include_stdout: false,
                include_stderr: false,
                include_source_context: true,
                only_survivors: false,
            },
            ReportDetail::Compact => ReportOptions {
                include_diff: false,
                include_stdout: false,
                include_stderr: false,
                include_source_context: true,
                only_survivors: true,
            },
        }
    }
}

/// The rendered shape of a `test-mutants` report. Replaces passing `"json"` /
/// `"sarif"` / … as strings: parsed once at the CLI/config boundary, then the
/// match over output shapes lives in one place (`render`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum ReportFormat {
    Json,
    Human,
    AgentTasksJson,
    AgentTasksMarkdown,
    GithubAnnotations,
    Sarif,
}

impl ReportFormat {
    /// Default detail level for this format. Survivor-only formats (agent tasks,
    /// sarif, github annotations) and the already-terse human format default to
    /// compact; the full JSON report defaults to normal.
    pub fn default_detail(self) -> ReportDetail {
        match self {
            ReportFormat::Json => ReportDetail::Normal,
            _ => ReportDetail::Compact,
        }
    }

    /// Render an enriched report into its textual form (trailing newline
    /// included for the serialized variants).
    pub fn render(self, report: &EnrichedRunReport) -> serde_json::Result<String> {
        Ok(match self {
            ReportFormat::Human => human(report),
            ReportFormat::AgentTasksJson => {
                let tasks = agent_tasks(report);
                let mut s = serde_json::to_string_pretty(&tasks)?;
                s.push('\n');
                s
            }
            ReportFormat::AgentTasksMarkdown => {
                let tasks = agent_tasks(report);
                agent_tasks_markdown(&tasks)
            }
            ReportFormat::GithubAnnotations => github_annotations(report),
            ReportFormat::Sarif => {
                let log = sarif(report);
                let mut s = serde_json::to_string_pretty(&log)?;
                s.push('\n');
                s
            }
            ReportFormat::Json => {
                let mut s = serde_json::to_string_pretty(report)?;
                s.push('\n');
                s
            }
        })
    }
}

/// Strip heavy fields and non-survivor outcomes from a report in place,
/// according to the resolved options. Summary counts (survived/timeout/error)
/// are left untouched so exit codes and totals stay correct.
pub fn apply_options(report: &mut EnrichedRunReport, opts: ReportOptions) {
    if opts.only_survivors {
        report
            .outcomes
            .retain(|o| matches!(o.outcome.status, MutantStatus::Survived));
    }
    for o in &mut report.outcomes {
        if !opts.include_diff {
            o.outcome.diff.clear();
        }
        if !opts.include_stdout {
            o.outcome.stdout.clear();
        }
        if !opts.include_stderr {
            o.outcome.stderr.clear();
        }
        if !opts.include_source_context {
            o.source_context = None;
        }
    }
    if !opts.include_source_context {
        for f in &mut report.functions {
            for s in &mut f.survived_mutants {
                s.source_context = None;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OozeExitCode {
    Success = 0,
    SurvivorsFound = 1,
    PreflightFailed = 2,
    InfrastructureProblem = 3,
    #[allow(dead_code)]
    UsageError = 4,
    #[allow(dead_code)]
    InternalError = 5,
}

impl OozeExitCode {
    pub fn code(self) -> i32 {
        self as i32
    }
}

pub fn exit_code_for_report(
    report: &EnrichedRunReport,
    no_fail_on_survivors: bool,
    allow_incomplete: bool,
) -> OozeExitCode {
    if (report.timeout > 0 || report.error > 0) && !allow_incomplete {
        return OozeExitCode::InfrastructureProblem;
    }
    if report.survived > 0 && !no_fail_on_survivors {
        return OozeExitCode::SurvivorsFound;
    }
    OozeExitCode::Success
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLog {
    pub version: String,
    #[serde(rename = "$schema")]
    pub schema: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifDriver {
    pub name: String,
    pub information_uri: String,
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRule {
    pub id: String,
    pub name: String,
    pub short_description: SarifMessage,
    pub full_description: SarifMessage,
    pub help: SarifMessage,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    pub rule_id: String,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifPhysicalLocation {
    pub artifact_location: SarifArtifactLocation,
    pub region: SarifRegion,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifArtifactLocation {
    pub uri: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRegion {
    pub start_line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<usize>,
}

pub fn sarif(report: &EnrichedRunReport) -> SarifLog {
    let mut rules: std::collections::BTreeMap<String, SarifRule> =
        std::collections::BTreeMap::new();
    let mut results = Vec::new();

    for o in &report.outcomes {
        if !matches!(o.outcome.status, MutantStatus::Survived) {
            continue;
        }
        let c = &o.outcome.candidate;
        let op = c.operator.as_str();
        let rule_id = format!("ooze.survived_mutant.{op}");

        rules.entry(rule_id.clone()).or_insert_with(|| SarifRule {
            id: rule_id.clone(),
            name: format!("Survived mutant: {op}"),
            short_description: SarifMessage {
                text: format!("{op} mutant survived"),
            },
            full_description: SarifMessage {
                text: "A mutation survived the test suite, suggesting missing behavioral coverage."
                    .to_string(),
            },
            help: SarifMessage {
                text: o.test_suggestion.as_ref().map_or_else(
                    || {
                        "Add a test that distinguishes the original behavior from the mutant."
                            .to_string()
                    },
                    |s| s.operator_hint.clone(),
                ),
            },
        });

        let text = o.test_suggestion.as_ref().map_or_else(
            || {
                format!(
                    "Survived mutant in `{}`: `{}` -> `{}`.",
                    c.function, c.original, c.replacement
                )
            },
            |s| s.prompt.clone(),
        );

        let uri = c
            .file
            .to_string_lossy()
            .trim_start_matches("./")
            .to_string();

        results.push(SarifResult {
            rule_id,
            level: "warning".to_string(),
            message: SarifMessage { text },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation { uri },
                    region: SarifRegion {
                        start_line: c.line,
                        start_column: Some(c.column + 1),
                    },
                },
            }],
        });
    }

    SarifLog {
        version: "2.1.0".to_string(),
        schema: "https://json.schemastore.org/sarif-2.1.0.json".to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "ooze".to_string(),
                    information_uri: "https://github.com/crocoder-dev/ooze".to_string(),
                    rules: rules.into_values().collect(),
                },
            },
            results,
        }],
    }
}

fn escape_github_annotation_value(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

pub fn github_annotations(report: &EnrichedRunReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for o in &report.outcomes {
        if !matches!(o.outcome.status, MutantStatus::Survived) {
            continue;
        }
        let c = &o.outcome.candidate;
        let title = escape_github_annotation_value("Ooze survived mutant");
        let file = escape_github_annotation_value(c.file.to_string_lossy().as_ref());
        let body = match &o.test_suggestion {
            Some(s) => format!(
                "{} {} -> {} survived in {}. {}",
                c.operator, c.original, c.replacement, c.function, s.prompt
            ),
            None => format!(
                "{} {} -> {} survived in {}",
                c.operator, c.original, c.replacement, c.function
            ),
        };
        let message = escape_github_annotation_value(&body);
        let _ = writeln!(
            out,
            "::warning file={},line={},col={},title={}::{}",
            file,
            c.line,
            c.column + 1,
            title,
            message
        );
    }
    out
}

pub fn agent_tasks_markdown(report: &AgentTaskReport) -> String {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fmt::Write;

    // Deduplicate by (file, line, column, operator, mutation) — keep highest priority_score per site
    type DedupeKey = (String, usize, usize, String, String);

    let mut out = String::new();
    let _ = writeln!(out, "# Mutation Testing Tasks\n");

    if report.tasks.is_empty() {
        out.push_str("No survived mutants. Nothing to write.\n");
        return out;
    }

    let mut best_idx: BTreeMap<DedupeKey, usize> = BTreeMap::new();
    let mut dup_counts: BTreeMap<DedupeKey, usize> = BTreeMap::new();

    for (i, t) in report.tasks.iter().enumerate() {
        let key: DedupeKey = (
            t.file.to_string_lossy().into_owned(),
            t.line,
            t.column,
            t.operator.as_str().to_string(),
            t.mutation.clone(),
        );
        *dup_counts.entry(key.clone()).or_insert(0) += 1;
        best_idx
            .entry(key)
            .and_modify(|j| {
                if report.tasks[i].priority_score > report.tasks[*j].priority_score {
                    *j = i;
                }
            })
            .or_insert(i);
    }

    let mut deduped: Vec<(&AgentTask, usize)> = best_idx
        .iter()
        .map(|(k, &idx)| (&report.tasks[idx], *dup_counts.get(k).unwrap_or(&1)))
        .collect();
    deduped.sort_by(|(a, _), (b, _)| {
        b.priority_score
            .partial_cmp(&a.priority_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let raw_count = report.tasks.len();
    let unique_mutations = deduped.len();

    // Unique locations = distinct (file, line) pairs across raw tasks
    let unique_locations: BTreeSet<(String, usize)> = report
        .tasks
        .iter()
        .map(|t| (t.file.to_string_lossy().into_owned(), t.line))
        .collect();
    let unique_location_count = unique_locations.len();

    let mut file_counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in &report.tasks {
        *file_counts
            .entry(t.file.to_string_lossy().into_owned())
            .or_insert(0) += 1;
    }
    let mut file_list: Vec<(String, usize)> = file_counts.into_iter().collect();
    file_list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    // --- Summary ---
    let _ = writeln!(out, "## Summary\n");
    let _ = writeln!(out, "- Raw mutations: {raw_count}");
    let _ = writeln!(
        out,
        "- Unique mutation records after dedupe: {unique_mutations}"
    );
    if unique_location_count < unique_mutations {
        let _ = writeln!(
            out,
            "- Unique source locations: {unique_location_count} (lines with multiple operators)"
        );
    }
    if raw_count > unique_mutations {
        let _ = writeln!(
            out,
            "- Duplicates removed: {}",
            raw_count - unique_mutations
        );
    }
    let _ = writeln!(out, "- Files affected: {}\n", file_list.len());
    out.push_str("Most affected files:\n\n");
    for (file, count) in file_list.iter().take(5) {
        let _ = writeln!(out, "- `{file}` ({count} mutations)");
    }
    out.push('\n');

    // --- Targets: emit function metrics once, sorted by priority desc ---
    let mut seen_funcs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut targets: Vec<&AgentTask> = Vec::new();
    for (t, _) in &deduped {
        let key = (t.file.to_string_lossy().into_owned(), t.function.clone());
        if seen_funcs.insert(key) {
            targets.push(t);
        }
    }
    // deduped is already sorted by priority_score desc, so targets preserves that order
    let _ = writeln!(
        out,
        "## Target{}\n",
        if targets.len() == 1 { "" } else { "s" }
    );
    for t in &targets {
        let crap = t.crap.map_or_else(|| "n/a".into(), |v| format!("{v:.1}"));
        let cc = t.cyclomatic.map_or_else(|| "n/a".into(), |v| v.to_string());
        let cov = t
            .coverage
            .map_or_else(|| "n/a".into(), |v| format!("{v:.1}%"));
        let _ = writeln!(out, "### `{}::{}`\n", t.file.display(), t.function);
        let _ = writeln!(
            out,
            "CRAP: **{crap}** | CC: **{cc}** | Coverage: **{cov}** | Priority: **{:.1}**\n",
            t.priority_score
        );
    }

    // Split into priority thirds (deduped is sorted desc by priority_score)
    let n = deduped.len();
    let high_end = (n / 3).max(1).min(n);
    let med_end = (n * 2 / 3).max(high_end + 1).min(n);
    let high = &deduped[..high_end];
    let medium = &deduped[high_end..med_end];
    let low = &deduped[med_end..];

    let single_target = targets.len() == 1;

    let sections: [(&str, &[(&AgentTask, usize)]); 3] =
        [("High", high), ("Medium", medium), ("Low", low)];

    for (label, bucket) in sections {
        if bucket.is_empty() {
            continue;
        }

        let _ = writeln!(out, "## {label} Priority\n");

        // Group by file → function
        let mut by_file: TasksByFile = BTreeMap::new();
        for &(t, dup) in bucket {
            by_file
                .entry(t.file.to_string_lossy().into_owned())
                .or_default()
                .entry(t.function.clone())
                .or_default()
                .push((t, dup));
        }

        let mut high_snippets: Vec<(usize, TaskRefs)> = Vec::new();

        for (file, funcs) in &by_file {
            if !single_target {
                let _ = writeln!(out, "### `{file}`\n");
            }

            for (func, func_tasks) in funcs {
                if !single_target {
                    let _ = writeln!(out, "#### `{func}`\n");
                }

                // Group mutations by line so same-location operators merge into one table row
                let mut by_line: BTreeMap<usize, TaskRefs> = BTreeMap::new();
                for &(t, dup) in func_tasks {
                    by_line.entry(t.line).or_default().push((t, dup));
                }

                let _ = writeln!(out, "| Line | Mutation(s) | Operator hint |");
                let _ = writeln!(out, "|-----:|-------------|---------------|");
                for (line, line_tasks) in &by_line {
                    let mutations: Vec<String> = line_tasks
                        .iter()
                        .map(|(t, dup)| {
                            if *dup > 1 {
                                format!("`{}` (col {}) (+{})", t.mutation, t.column, dup - 1)
                            } else {
                                format!("`{}` (col {})", t.mutation, t.column)
                            }
                        })
                        .collect();
                    let hints: Vec<&str> = {
                        let mut seen: BTreeSet<&str> = BTreeSet::new();
                        line_tasks
                            .iter()
                            .filter_map(|(t, _)| {
                                let h = t.operator_hint.as_str();
                                seen.insert(h).then_some(h)
                            })
                            .collect()
                    };
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} |",
                        line,
                        mutations.join(", "),
                        hints.join(" "),
                    );

                    if label == "High" {
                        high_snippets.push((*line, line_tasks.clone()));
                    }
                }
                out.push('\n');
            }
        }

        // Code context only for High priority, collected above to keep tables
        // uninterrupted. `high_snippets` is only populated in the High section
        // (see above), so a non-empty vec already implies `label == "High"`.
        if !high_snippets.is_empty() {
            let _ = writeln!(out, "### Context\n");
            for (line, line_tasks) in &high_snippets {
                if let Some(ctx) = line_tasks
                    .iter()
                    .find_map(|(t, _)| t.source_context.as_ref())
                {
                    let mutations: Vec<String> = line_tasks
                        .iter()
                        .map(|(t, _)| format!("{} (col {})", t.mutation, t.column))
                        .collect();
                    let _ = writeln!(out, "**Line {}** — {}\n", line, mutations.join(", "));
                    out.push_str("```text\n");
                    out.push_str(&ctx.snippet);
                    out.push_str("```\n\n");
                    for (t, _) in line_tasks {
                        let _ = writeln!(out, "> {}\n", t.prompt);
                    }
                }
            }
        }
    }

    out
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OperatorMutationSummary {
    pub operator: OperatorName,
    pub total: usize,
    pub killed: usize,
    pub survived: usize,
    pub timeout: usize,
    pub error: usize,
    pub mutation_score: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceContext {
    pub start_line: usize,
    pub end_line: usize,
    pub snippet: String,
}

pub fn source_context_for_candidate(
    repo_root: &Path,
    candidate: &MutationCandidate,
    radius: usize,
) -> Option<SourceContext> {
    use std::fmt::Write;
    if radius == 0 {
        return None;
    }

    let text = std::fs::read_to_string(&candidate.file)
        .or_else(|_| {
            let rel = candidate
                .file
                .strip_prefix(repo_root)
                .unwrap_or(candidate.file.as_path());
            std::fs::read_to_string(repo_root.join(rel))
        })
        .ok()?;

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let line = candidate.line.max(1);
    let start = line.saturating_sub(radius).max(1);
    let end = (line + radius).min(lines.len());

    let mut snippet = String::new();
    for n in start..=end {
        let marker = if n == line { ">" } else { " " };
        let content = lines.get(n - 1).copied().unwrap_or("");
        let _ = writeln!(snippet, "{marker} {n:>4} | {content}");
    }

    Some(SourceContext {
        start_line: start,
        end_line: end,
        snippet,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TestSuggestion {
    pub focus: String,
    pub operator_hint: String,
    pub prompt: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SurvivedMutant {
    pub candidate: MutationCandidate,
    pub test_suggestion: TestSuggestion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_context: Option<SourceContext>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FunctionMutationSummary {
    pub file: PathBuf,
    pub function: String,
    pub cyclomatic: Option<usize>,
    pub coverage: Option<f64>,
    pub crap: Option<f64>,

    pub total: usize,
    pub killed: usize,
    pub survived: usize,
    pub timeout: usize,
    pub error: usize,

    pub mutation_score: Option<f64>,
    pub priority_score: f64,

    pub survived_mutants: Vec<SurvivedMutant>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichedOutcome {
    #[serde(flatten)]
    pub outcome: MutantOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_suggestion: Option<TestSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_context: Option<SourceContext>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichedRunReport {
    pub total: usize,
    pub killed: usize,
    pub survived: usize,
    pub timeout: usize,
    pub error: usize,
    pub mutation_score: Option<f64>,
    pub operators: Vec<OperatorMutationSummary>,
    pub functions: Vec<FunctionMutationSummary>,
    pub outcomes: Vec<EnrichedOutcome>,
}

pub fn suggest(candidate: &MutationCandidate) -> TestSuggestion {
    let info = candidate.operator.info();
    let file = candidate.file.display();
    let func = &candidate.function;
    // Rendered as "<line>, column <col>" so every "at line {line}" prompt below
    // pinpoints the mutation site, not just the line.
    let line = format!("{}, column {}", candidate.line, candidate.column + 1);
    let original = &candidate.original;
    let replacement = &candidate.replacement;

    let prompt = match candidate.operator {
        OperatorName::ComparisonBoundary => format!(
            "Add a boundary test for `{func}` in `{file}` at the exact threshold value. The test should fail if `{original}` at line {line} is changed to `{replacement}`."
        ),
        OperatorName::ComparisonNegation => format!(
            "Add a test for `{func}` in `{file}` that covers inputs on both sides of the comparison. The test should fail if `{original}` at line {line} is changed to `{replacement}`."
        ),
        OperatorName::NegateEquality => format!(
            "Add a test for `{func}` in `{file}` that covers both equal and non-equal inputs. The test should fail if `{original}` at line {line} is changed to `{replacement}`."
        ),
        OperatorName::SwapLogical => format!(
            "Add a truth-table-style test for `{func}` in `{file}`. Cover cases where the left and right side of the condition differ. The test should fail if `{original}` at line {line} is changed to `{replacement}`."
        ),
        OperatorName::RemoveNot => format!(
            "Add a test for `{func}` in `{file}` that exercises the negative path of the condition at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::SwapBoolean => format!(
            "Add a test for `{func}` in `{file}` that asserts the boolean branch/result at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::IntegerZeroOne => format!(
            "Add a test for `{func}` in `{file}` that distinguishes counts of 0 vs 1 around line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::RangeInclusiveExclusive => format!(
            "Add a test for `{func}` in `{file}` that exercises the final element of the range at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::SwapPredicateMethod => format!(
            "Add a test for `{func}` in `{file}` that distinguishes the outcomes of `{original}()` and `{replacement}()` at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::NegatePredicateMethod => format!(
            "Add a test for `{func}` in `{file}` that covers both the matching and non-matching cases of `{original}` at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::ReturnBoolean => format!(
            "Add a test for `{func}` in `{file}` that asserts the boolean returned at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::IsNoneNegation => format!(
            "Add a test for `{func}` in `{file}` that covers both the None and non-None case at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::InNegation => format!(
            "Add a test for `{func}` in `{file}` that covers both a member and a non-member input at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::TruthinessNegation => format!(
            "Add a test for `{func}` in `{file}` that drives the condition at line {line} both truthy and falsy. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::LenZeroBoundary => format!(
            "Add empty and non-empty collection tests for `{func}` in `{file}` at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::DictGetDefaultRemoval => format!(
            "Add a test for `{func}` in `{file}` that exercises a missing key so the default at line {line} matters. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::ComprehensionFilterRemoval => format!(
            "Add a test for `{func}` in `{file}` with inputs the filter at line {line} is meant to exclude. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::NoneReturn => format!(
            "Add a test for `{func}` in `{file}` that asserts the concrete value returned at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::EmptyCollectionLiteral => format!(
            "Add a test for `{func}` in `{file}` that asserts the contents of the collection at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::IteratorAnyAll => format!(
            "Add a test for `{func}` in `{file}` with a mix of matching and non-matching elements so `{original}` and `{replacement}` differ at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::MatchBoolPattern => format!(
            "Add a test for `{func}` in `{file}` that drives the match scrutinee at line {line} both true and false. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::OkErrBoolean => format!(
            "Add a test for `{func}` in `{file}` that asserts the boolean wrapped at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::SomeBoolean => format!(
            "Add a test for `{func}` in `{file}` that asserts the boolean wrapped in the option at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::OptionSomeNone => format!(
            "Add a test for `{func}` in `{file}` that distinguishes a present value from None at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::RemoveTry => format!(
            "Add a test for `{func}` in `{file}` that drives the error path of the `?` at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::UnwrapToUnwrapOrDefault | OperatorName::ExpectToUnwrapOrDefault => format!(
            "Add a test for `{func}` in `{file}` that exercises the None/Err case at line {line} so the panic-vs-default behavior differs. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::MinMaxSwap => format!(
            "Add a test for `{func}` in `{file}` where the smallest and largest values differ so `{original}` and `{replacement}` disagree at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::MatchWildcardToPanic => format!(
            "Add a test for `{func}` in `{file}` that exercises the fallback match arm at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::EmptyVecMacro => format!(
            "Add a test for `{func}` in `{file}` that asserts the contents of the vector at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::SaturatingCheckedSwap => format!(
            "Add a test for `{func}` in `{file}` at the overflow boundary so saturating and checked behavior differ at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::StringBoundaryMethodSwap => format!(
            "Add a test for `{func}` in `{file}` covering both matching and non-matching prefixes/suffixes so `{original}` and `{replacement}` differ at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::IncludesNegation => format!(
            "Add a test for `{func}` in `{file}` covering both a present and an absent value at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::SortedReverseFlip => format!(
            "Add a test for `{func}` in `{file}` that asserts the exact ordering at line {line}, not just membership. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::DictGetToIndex => format!(
            "Add a test for `{func}` in `{file}` that distinguishes missing-key from present-key behavior at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::NullishCoalescingRemoval => format!(
            "Add a test for `{func}` in `{file}` where the left side of the `??` at line {line} is null or undefined so the fallback matters. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::OptionalChainingRemoval => format!(
            "Add a test for `{func}` in `{file}` where the receiver of the `?.` at line {line} is null or undefined. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::TernaryArmSwap => format!(
            "Add a test for `{func}` in `{file}` that drives both branches of the ternary at line {line} and asserts the returned value. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::TernaryConditionNegation => format!(
            "Add a test for `{func}` in `{file}` that drives the ternary at line {line} with a condition value on each side and asserts the returned value. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::ArrayEmptyLiteral => format!(
            "Add a test for `{func}` in `{file}` that asserts the exact contents of the array at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::ObjectEmptyLiteral => format!(
            "Add a test for `{func}` in `{file}` that asserts the required properties and values of the object at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::StringEmptyLiteral => format!(
            "Add a test for `{func}` in `{file}` that asserts the exact string at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::AwaitRemoval => format!(
            "Add an async test for `{func}` in `{file}` that asserts the resolved value and ordering around line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::SwapArithmetic => format!(
            "Add a test for `{func}` in `{file}` that asserts the exact computed value at line {line}, with operands chosen so `{original}` and `{replacement}` differ. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::SwapAssignment => format!(
            "Add a test for `{func}` in `{file}` that asserts the accumulated value after line {line} runs. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::RemoveUnaryMinus => format!(
            "Add a test for `{func}` in `{file}` with a strictly positive input so the sign at line {line} matters. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::PlusToMinus => format!(
            "Add a test for `{func}` in `{file}` with a nonzero input so the sign flip at line {line} is visible. The test should fail if `{original}` is changed to `{replacement}`."
        ),
    };

    TestSuggestion {
        focus: info.category.to_string(),
        operator_hint: info.test_hint.to_string(),
        prompt,
    }
}

fn priority_score(crap: Option<f64>, coverage: Option<f64>, mutation_score: Option<f64>) -> f64 {
    let crap = crap.unwrap_or(0.0);
    let coverage = coverage.unwrap_or(0.0);
    let mutation_gap = match mutation_score {
        Some(s) => 100.0 - s,
        None => 0.0,
    };
    crap + mutation_gap + (100.0 - coverage) * 0.25
}

pub fn enrich(
    report: MutationRunReport,
    crap_entries: &[CrapEntry],
    repo_root: &Path,
    context_lines: usize,
) -> EnrichedRunReport {
    // Crap entries come from a separate scan than the mutation outcomes, so the
    // two can spell the same file differently. Join on source identity, not on
    // the raw path string. The display path kept for the summary is the one the
    // outcome carried, so output spelling is unchanged.
    let crap_index: HashMap<(FileKey, String), &CrapEntry> = crap_entries
        .iter()
        .map(|e| {
            (
                (
                    FileKey::resolve_under(repo_root, &e.file),
                    e.function.clone(),
                ),
                e,
            )
        })
        .collect();

    let mut buckets: HashMap<(FileKey, String), (PathBuf, Vec<MutantOutcome>)> = HashMap::new();
    for outcome in &report.outcomes {
        let key = (
            FileKey::resolve_under(repo_root, &outcome.candidate.file),
            outcome.candidate.function.clone(),
        );
        let slot = buckets
            .entry(key)
            .or_insert_with(|| (outcome.candidate.file.clone(), Vec::new()));
        slot.1.push(outcome.clone());
    }

    let mut functions: Vec<FunctionMutationSummary> = buckets
        .into_iter()
        .map(|((file_key, function), (file, outcomes))| {
            let total = outcomes.len();
            let killed = outcomes
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Killed))
                .count();
            let survived = outcomes
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Survived))
                .count();
            let timeout = outcomes
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Timeout))
                .count();
            let error = outcomes
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Error))
                .count();

            let meaningful = killed + survived;
            let mutation_score = if meaningful == 0 {
                None
            } else {
                Some(killed as f64 / meaningful as f64 * 100.0)
            };

            let entry = crap_index.get(&(file_key, function.clone())).copied();
            let crap = entry.map(|e| e.crap);
            let coverage = entry.map(|e| e.coverage);
            let cyclomatic = entry.map(|e| e.cyclomatic);

            let survived_mutants: Vec<SurvivedMutant> = outcomes
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Survived))
                .map(|o| SurvivedMutant {
                    candidate: o.candidate.clone(),
                    test_suggestion: suggest(&o.candidate),
                    source_context: source_context_for_candidate(
                        repo_root,
                        &o.candidate,
                        context_lines,
                    ),
                })
                .collect();

            FunctionMutationSummary {
                file,
                function,
                cyclomatic,
                coverage,
                crap,
                total,
                killed,
                survived,
                timeout,
                error,
                mutation_score,
                priority_score: priority_score(crap, coverage, mutation_score),
                survived_mutants,
            }
        })
        .collect();

    functions.sort_by(|a, b| {
        b.priority_score
            .partial_cmp(&a.priority_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.function.cmp(&b.function))
    });

    let meaningful = report.killed + report.survived;
    let mutation_score = if meaningful == 0 {
        None
    } else {
        Some(report.killed as f64 / meaningful as f64 * 100.0)
    };

    let mut op_buckets: HashMap<OperatorName, Vec<&MutantOutcome>> = HashMap::new();
    for o in &report.outcomes {
        op_buckets.entry(o.candidate.operator).or_default().push(o);
    }
    let mut operators: Vec<OperatorMutationSummary> = op_buckets
        .into_iter()
        .map(|(op, outs)| {
            let total = outs.len();
            let killed = outs
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Killed))
                .count();
            let survived = outs
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Survived))
                .count();
            let timeout = outs
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Timeout))
                .count();
            let error = outs
                .iter()
                .filter(|o| matches!(o.status, MutantStatus::Error))
                .count();
            let meaningful = killed + survived;
            let mutation_score = if meaningful == 0 {
                None
            } else {
                Some(killed as f64 / meaningful as f64 * 100.0)
            };
            OperatorMutationSummary {
                operator: op,
                total,
                killed,
                survived,
                timeout,
                error,
                mutation_score,
            }
        })
        .collect();
    operators.sort_by(|a, b| {
        b.total
            .cmp(&a.total)
            .then_with(|| a.operator.cmp(&b.operator))
    });

    let outcomes: Vec<EnrichedOutcome> = report
        .outcomes
        .into_iter()
        .map(|o| {
            let (test_suggestion, source_context) = if matches!(o.status, MutantStatus::Survived) {
                (
                    Some(suggest(&o.candidate)),
                    source_context_for_candidate(repo_root, &o.candidate, context_lines),
                )
            } else {
                (None, None)
            };
            EnrichedOutcome {
                outcome: o,
                test_suggestion,
                source_context,
            }
        })
        .collect();

    EnrichedRunReport {
        total: report.total,
        killed: report.killed,
        survived: report.survived,
        timeout: report.timeout,
        error: report.error,
        mutation_score,
        operators,
        functions,
        outcomes,
    }
}

pub fn human(report: &EnrichedRunReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    let score = report
        .mutation_score
        .map_or_else(|| "n/a".to_string(), |s| format!("{s:.1}%"));

    let _ = writeln!(
        out,
        "Mutation score: {}  ({} killed, {} survived, {} timeout, {} error)",
        score, report.killed, report.survived, report.timeout, report.error
    );
    out.push('\n');

    if !report.operators.is_empty() {
        out.push_str("Per-operator:\n");
        for op in &report.operators {
            let ms = op
                .mutation_score
                .map_or_else(|| "n/a".into(), |v| format!("{v:.1}%"));
            let _ = writeln!(
                out,
                "  {:<18} total {:>3}  killed {:>3}  survived {:>3}  score {}",
                op.operator.as_str(),
                op.total,
                op.killed,
                op.survived,
                ms
            );
        }
        out.push('\n');
    }

    let top: Vec<_> = report
        .functions
        .iter()
        .filter(|f| f.survived > 0 || f.timeout > 0 || f.error > 0)
        .take(10)
        .collect();

    if top.is_empty() {
        out.push_str("No weakly-tested functions in this run.\n");
        return out;
    }

    out.push_str("Top test targets:\n\n");
    for (i, f) in top.iter().enumerate() {
        let crap = f.crap.map_or_else(|| "n/a".into(), |v| format!("{v:.1}"));
        let cc = f.cyclomatic.map_or_else(|| "n/a".into(), |v| v.to_string());
        let cov = f
            .coverage
            .map_or_else(|| "n/a".into(), |v| format!("{v:.1}%"));
        let ms = f
            .mutation_score
            .map_or_else(|| "n/a".into(), |v| format!("{v:.1}%"));

        let _ = writeln!(out, "{}. {}::{}", i + 1, f.file.display(), f.function);
        let _ = writeln!(
            out,
            "   CRAP {} | CC {} | coverage {} | mutation score {} | priority {:.1}",
            crap, cc, cov, ms, f.priority_score
        );

        if !f.survived_mutants.is_empty() {
            out.push_str("   Survived mutants:\n");
            for s in &f.survived_mutants {
                let _ = writeln!(
                    out,
                    "   - {} at line {}, column {}: {:?} -> {:?}",
                    s.candidate.operator,
                    s.candidate.line,
                    s.candidate.column + 1,
                    s.candidate.original,
                    s.candidate.replacement
                );
                let _ = writeln!(out, "     suggestion: {}", s.test_suggestion.prompt);
                if let Some(ctx) = &s.source_context {
                    out.push_str("     context:\n");
                    for snippet_line in ctx.snippet.lines() {
                        let _ = writeln!(out, "       {snippet_line}");
                    }
                }
            }
        }
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Language, OperatorCategory};
    use std::path::PathBuf;

    fn make_task(
        file: &str,
        line: usize,
        operator: OperatorName,
        mutation: &str,
        priority: f64,
    ) -> AgentTask {
        AgentTask {
            id: format!("task-{priority}"),
            file: PathBuf::from(file),
            function: "test_fn".to_string(),
            line,
            column: 1,
            operator,
            mutation: mutation.to_string(),
            focus: String::new(),
            operator_hint: String::new(),
            prompt: String::new(),
            cyclomatic: None,
            coverage: None,
            crap: None,
            priority_score: priority,
            source_context: None,
        }
    }

    #[test]
    fn agent_tasks_markdown_dedup_keeps_higher_priority() {
        // Two tasks with identical dedup key; second has higher priority_score.
        let low = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            1.0,
        );
        let high = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            99.0,
        );
        let report = AgentTaskReport {
            tasks: vec![low, high],
        };
        let md = agent_tasks_markdown(&report);
        // The high-priority task's id contains "99" — it must appear in the output.
        assert!(
            md.contains("99"),
            "dedup should keep the higher-priority task; got:\n{md}"
        );
    }

    #[test]
    fn agent_tasks_markdown_dedup_keeps_higher_priority_when_first_is_higher() {
        // First task has higher priority_score; second is lower.
        let high = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            99.0,
        );
        let low = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            1.0,
        );
        let report = AgentTaskReport {
            tasks: vec![high, low],
        };
        let md = agent_tasks_markdown(&report);
        assert!(
            md.contains("99"),
            "dedup should keep the higher-priority task; got:\n{md}"
        );
    }

    #[test]
    fn agent_tasks_markdown_empty_report() {
        let report = AgentTaskReport { tasks: vec![] };
        let md = agent_tasks_markdown(&report);
        assert!(md.contains("No survived mutants"));
    }

    fn make_task_ctx(
        file: &str,
        line: usize,
        operator: OperatorName,
        mutation: &str,
        priority: f64,
        prompt: &str,
    ) -> AgentTask {
        let mut t = make_task(file, line, operator, mutation, priority);
        t.prompt = prompt.to_string();
        t.source_context = Some(SourceContext {
            start_line: line,
            end_line: line,
            snippet: format!("ctx for line {line}\n"),
        });
        t
    }

    #[test]
    fn dedup_tiebreak_keeps_first_on_equal_priority() {
        // Kills line 435 `> -> >=`: with equal priority_score, `>` keeps the
        // first task while `>=` switches to the later one. Both share a dedup
        // key but differ in operator_hint, which is printed in the table.
        let mut a = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            5.0,
        );
        a.operator_hint = "ALPHAHINT".to_string();
        let mut b = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            5.0,
        );
        b.operator_hint = "BETAHINT".to_string();
        let report = AgentTaskReport { tasks: vec![a, b] };
        let md = agent_tasks_markdown(&report);
        assert!(
            md.contains("ALPHAHINT"),
            "should keep first task's hint:\n{md}"
        );
        assert!(
            !md.contains("BETAHINT"),
            "should not switch to later equal-priority task:\n{md}"
        );
    }

    #[test]
    fn summary_reports_duplicates_and_dup_marker() {
        // raw_count > unique_mutations prints "Duplicates removed" (line 480) and
        // dup > 1 prints the "(+N)" marker (line 568).
        let a = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            5.0,
        );
        let b = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            5.0,
        );
        let report = AgentTaskReport { tasks: vec![a, b] };
        let md = agent_tasks_markdown(&report);
        assert!(
            md.contains("Duplicates removed: 1"),
            "raw>unique should print duplicates line:\n{md}"
        );
        assert!(md.contains("(+1)"), "dup>1 should print +N marker:\n{md}");
    }

    #[test]
    fn summary_omits_duplicates_when_none() {
        // raw_count == unique_mutations: no "Duplicates removed" (line 480 `>=`)
        // and every dup == 1, so no "(+" marker (line 568 `>=`).
        let a = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            5.0,
        );
        let b = make_task(
            "src/lib.rs",
            20,
            OperatorName::NegateEquality,
            "== -> !=",
            4.0,
        );
        let report = AgentTaskReport { tasks: vec![a, b] };
        let md = agent_tasks_markdown(&report);
        assert!(
            !md.contains("Duplicates removed"),
            "no dups -> no duplicates line:\n{md}"
        );
        assert!(!md.contains("(+"), "no dups -> no +N marker:\n{md}");
    }

    #[test]
    fn summary_reports_unique_locations_for_multiple_ops_per_line() {
        // Two mutations on the same line: unique_location_count(1) <
        // unique_mutations(2) prints the "Unique source locations" line
        // (line 474 `< -> >=`).
        let a = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            5.0,
        );
        let b = make_task(
            "src/lib.rs",
            10,
            OperatorName::ComparisonBoundary,
            "> -> >=",
            4.0,
        );
        let report = AgentTaskReport { tasks: vec![a, b] };
        let md = agent_tasks_markdown(&report);
        assert!(
            md.contains("Unique source locations"),
            "multiple ops per line should list unique locations:\n{md}"
        );
    }

    #[test]
    fn summary_omits_unique_locations_for_one_op_per_line() {
        // One mutation per distinct line: count == unique, line omitted
        // (line 474 `< -> <=`).
        let a = make_task(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            5.0,
        );
        let b = make_task(
            "src/lib.rs",
            20,
            OperatorName::NegateEquality,
            "== -> !=",
            4.0,
        );
        let report = AgentTaskReport { tasks: vec![a, b] };
        let md = agent_tasks_markdown(&report);
        assert!(
            !md.contains("Unique source locations"),
            "one op per line should omit unique locations:\n{md}"
        );
    }

    #[test]
    fn single_target_suppresses_headers_and_emits_high_context() {
        // One file+function -> single_target == true, so per-section file/function
        // headers are suppressed (line 521 `==`, lines 547/552 `!single_target`).
        // The non-empty High bucket prints its section (line 527) and, since the
        // High task carries source_context, a Context block with its prompt
        // (line 593 `==`, line 602 negations).
        let a = make_task_ctx(
            "src/lib.rs",
            10,
            OperatorName::NegateEquality,
            "== -> !=",
            9.0,
            "PROMPT_A",
        );
        let b = make_task_ctx(
            "src/lib.rs",
            20,
            OperatorName::ComparisonBoundary,
            "> -> >=",
            5.0,
            "PROMPT_B",
        );
        let c = make_task_ctx(
            "src/lib.rs",
            30,
            OperatorName::ComparisonNegation,
            "< -> >=",
            1.0,
            "PROMPT_C",
        );
        let report = AgentTaskReport {
            tasks: vec![a, b, c],
        };
        let md = agent_tasks_markdown(&report);

        assert!(
            md.contains("## High Priority"),
            "high section present (527):\n{md}"
        );
        assert!(
            !md.contains("### `src/lib.rs`\n"),
            "file header suppressed for single target:\n{md}"
        );
        assert!(
            !md.contains("#### "),
            "function header suppressed for single target:\n{md}"
        );
        assert!(
            md.contains("### Context"),
            "high context section emitted:\n{md}"
        );
        assert!(
            md.contains("PROMPT_A"),
            "high task's prompt appears in its Context block:\n{md}"
        );
    }

    fn make_outcome(status: MutantStatus) -> EnrichedOutcome {
        let candidate = MutationCandidate {
            id: "m1".into(),
            file: PathBuf::from("src/lib.rs"),
            language: Language::Rust,
            function: "f".into(),
            operator: OperatorName::NegateEquality,
            operator_category: OperatorCategory::Equality,
            implementation: "rust.negate_equality".into(),
            line: 1,
            column: 0,
            start_byte: 0,
            end_byte: 0,
            original: "==".into(),
            replacement: "!=".into(),
            description: String::new(),
        };
        EnrichedOutcome {
            outcome: MutantOutcome {
                candidate,
                status,
                exit_code: Some(0),
                duration_ms: 0,
                diff: "some diff".into(),
                stdout: "some stdout".into(),
                stderr: "some stderr".into(),
            },
            test_suggestion: None,
            source_context: Some(SourceContext {
                start_line: 1,
                end_line: 1,
                snippet: "ctx".into(),
            }),
        }
    }

    fn report_with(outcomes: Vec<EnrichedOutcome>) -> EnrichedRunReport {
        EnrichedRunReport {
            total: outcomes.len(),
            killed: 0,
            survived: 0,
            timeout: 0,
            error: 0,
            mutation_score: None,
            operators: vec![],
            functions: vec![],
            outcomes,
        }
    }

    #[test]
    fn compact_drops_diff_output_and_non_survivors() {
        let mut report = report_with(vec![
            make_outcome(MutantStatus::Survived),
            make_outcome(MutantStatus::Killed),
        ]);
        apply_options(
            &mut report,
            ReportOptions::from_detail(ReportDetail::Compact),
        );
        assert_eq!(report.outcomes.len(), 1, "only the survivor should remain");
        let o = &report.outcomes[0].outcome;
        assert!(o.diff.is_empty());
        assert!(o.stdout.is_empty());
        assert!(o.stderr.is_empty());
    }

    #[test]
    fn normal_keeps_diff_but_drops_probe_output() {
        let mut report = report_with(vec![make_outcome(MutantStatus::Killed)]);
        apply_options(
            &mut report,
            ReportOptions::from_detail(ReportDetail::Normal),
        );
        assert_eq!(report.outcomes.len(), 1, "non-survivors are kept");
        let o = &report.outcomes[0].outcome;
        assert_eq!(o.diff, "some diff");
        assert!(o.stdout.is_empty());
        assert!(o.stderr.is_empty());
    }

    #[test]
    fn full_keeps_everything() {
        let mut report = report_with(vec![make_outcome(MutantStatus::Killed)]);
        apply_options(&mut report, ReportOptions::from_detail(ReportDetail::Full));
        let o = &report.outcomes[0].outcome;
        assert_eq!(o.diff, "some diff");
        assert_eq!(o.stdout, "some stdout");
        assert_eq!(o.stderr, "some stderr");
    }

    #[test]
    fn flag_overrides_compose_with_detail() {
        // Start from full, then override individual fields off.
        let mut opts = ReportOptions::from_detail(ReportDetail::Full);
        opts.include_diff = false;
        opts.only_survivors = true;
        let mut report = report_with(vec![
            make_outcome(MutantStatus::Survived),
            make_outcome(MutantStatus::Timeout),
        ]);
        apply_options(&mut report, opts);
        assert_eq!(report.outcomes.len(), 1);
        let o = &report.outcomes[0].outcome;
        assert!(o.diff.is_empty());
        assert_eq!(o.stdout, "some stdout", "stdout untouched by --no-diff");
    }

    #[test]
    fn default_detail_per_format() {
        assert_eq!(ReportFormat::Human.default_detail(), ReportDetail::Compact);
        assert_eq!(ReportFormat::Sarif.default_detail(), ReportDetail::Compact);
        assert_eq!(
            ReportFormat::AgentTasksJson.default_detail(),
            ReportDetail::Compact
        );
        assert_eq!(
            ReportFormat::AgentTasksMarkdown.default_detail(),
            ReportDetail::Compact
        );
        assert_eq!(
            ReportFormat::GithubAnnotations.default_detail(),
            ReportDetail::Compact
        );
        assert_eq!(ReportFormat::Json.default_detail(), ReportDetail::Normal);
    }

    #[test]
    fn enrich_joins_crap_across_path_spellings() {
        use crate::core::{CrapEntry, MutationRunReport};

        // A real file on disk so both spellings canonicalize to the same identity.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src/x.rs"), "fn f() {}\n").unwrap();

        // The mutation outcome carries an absolute path (as a run produces);
        let mut outcome = make_outcome(MutantStatus::Survived);
        outcome.outcome.candidate.file = root.join("src/x.rs");
        outcome.outcome.candidate.function = "f".into();

        // ...while the crap scan spelled the same file repo-relative. A raw
        // PathBuf join would miss this; the FileKey identity join must not.
        let crap = CrapEntry {
            file: PathBuf::from("src/x.rs"),
            language: Language::Rust,
            function: "f".into(),
            line: 1,
            cyclomatic: 4,
            coverage: 25.0,
            crap: 42.0,
        };

        let report = MutationRunReport::from_outcomes(vec![outcome.outcome]);
        let enriched = enrich(report, &[crap], root, 0);

        let func = enriched
            .functions
            .iter()
            .find(|f| f.function == "f")
            .expect("function summary present");
        assert_eq!(
            func.crap,
            Some(42.0),
            "crap must join despite path spelling"
        );
        assert_eq!(func.coverage, Some(25.0));
        assert_eq!(func.cyclomatic, Some(4));
    }
}
