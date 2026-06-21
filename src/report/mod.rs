use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::{
    CrapEntry, MutantOutcome, MutantStatus, MutationCandidate, MutationRunReport, OperatorName,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentTask {
    pub id: String,
    pub file: PathBuf,
    pub function: String,
    pub line: usize,
    pub operator: OperatorName,
    pub mutation: String,
    pub focus: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_context: Option<SourceContext>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentTaskReport {
    pub tasks: Vec<AgentTask>,
}

pub fn agent_tasks(report: &EnrichedRunReport) -> AgentTaskReport {
    let mut tasks = Vec::new();
    for o in &report.outcomes {
        if !matches!(o.outcome.status, MutantStatus::Survived) {
            continue;
        }
        let Some(suggestion) = &o.test_suggestion else {
            continue;
        };
        tasks.push(AgentTask {
            id: format!("test-task-{:03}", tasks.len() + 1),
            file: o.outcome.candidate.file.clone(),
            function: o.outcome.candidate.function.clone(),
            line: o.outcome.candidate.line,
            operator: o.outcome.candidate.operator,
            mutation: format!(
                "{} -> {}",
                o.outcome.candidate.original, o.outcome.candidate.replacement
            ),
            focus: suggestion.focus.clone(),
            prompt: suggestion.prompt.clone(),
            source_context: o.source_context.clone(),
        });
    }
    AgentTaskReport { tasks }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OozeExitCode {
    Success = 0,
    SurvivorsFound = 1,
    PreflightFailed = 2,
    InfrastructureProblem = 3,
    UsageError = 4,
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
    let mut rules: std::collections::BTreeMap<String, SarifRule> = std::collections::BTreeMap::new();
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
                text: o
                    .test_suggestion
                    .as_ref()
                    .map(|s| s.operator_hint.clone())
                    .unwrap_or_else(|| {
                        "Add a test that distinguishes the original behavior from the mutant."
                            .to_string()
                    }),
            },
        });

        let text = o
            .test_suggestion
            .as_ref()
            .map(|s| s.prompt.clone())
            .unwrap_or_else(|| {
                format!(
                    "Survived mutant in `{}`: `{}` -> `{}`.",
                    c.function, c.original, c.replacement
                )
            });

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
            "::warning file={},line={},title={}::{}",
            file, c.line, title, message
        );
    }
    out
}

pub fn agent_tasks_markdown(report: &AgentTaskReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "# Mutation Testing Tasks\n");
    if report.tasks.is_empty() {
        out.push_str("No survived mutants. Nothing to write.\n");
        return out;
    }
    for (i, t) in report.tasks.iter().enumerate() {
        let _ = writeln!(out, "## {}. `{}`\n", i + 1, t.function);
        let _ = writeln!(out, "File: `{}:{}`  ", t.file.display(), t.line);
        let _ = writeln!(out, "Operator: `{}`  ", t.operator.as_str());
        let _ = writeln!(out, "Mutation: `{}`  ", t.mutation);
        let _ = writeln!(out, "Focus: {}\n", t.focus);
        let _ = writeln!(out, "{}\n", t.prompt);
        if let Some(ctx) = &t.source_context {
            out.push_str("```text\n");
            out.push_str(&ctx.snippet);
            out.push_str("```\n\n");
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
        snippet.push_str(&format!("{marker} {:>4} | {content}\n", n));
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
    let line = candidate.line;
    let original = &candidate.original;
    let replacement = &candidate.replacement;

    let prompt = match candidate.operator {
        OperatorName::SwapComparison => format!(
            "Add a boundary-focused test for `{func}` in `{file}`. The test should fail if `{original}` at line {line} is changed to `{replacement}`."
        ),
        OperatorName::NegateEquality => format!(
            "Add a test for `{func}` in `{file}` that covers both equal and non-equal inputs. The test should fail if `{original}` at line {line} is changed to `{replacement}`."
        ),
        OperatorName::SwapLogical => format!(
            "Add a truth-table-style test for `{func}` in `{file}`. Cover cases where the left and right side of the condition differ. The test should fail if `{original}` at line {line} is changed to `{replacement}`."
        ),
        OperatorName::SwapBoolean => format!(
            "Add a test for `{func}` in `{file}` that asserts the boolean branch/result at line {line}. The test should fail if `{original}` is changed to `{replacement}`."
        ),
        OperatorName::IntegerZeroOne => format!(
            "Add a test for `{func}` in `{file}` that distinguishes counts of 0 vs 1 around line {line}. The test should fail if `{original}` is changed to `{replacement}`."
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
    let crap_index: HashMap<(PathBuf, String), &CrapEntry> = crap_entries
        .iter()
        .map(|e| ((e.file.clone(), e.function.clone()), e))
        .collect();

    let mut buckets: HashMap<(PathBuf, String), Vec<MutantOutcome>> = HashMap::new();
    for outcome in &report.outcomes {
        buckets
            .entry((outcome.candidate.file.clone(), outcome.candidate.function.clone()))
            .or_default()
            .push(outcome.clone());
    }

    let mut functions: Vec<FunctionMutationSummary> = buckets
        .into_iter()
        .map(|((file, function), outcomes)| {
            let total = outcomes.len();
            let killed = outcomes.iter().filter(|o| matches!(o.status, MutantStatus::Killed)).count();
            let survived = outcomes.iter().filter(|o| matches!(o.status, MutantStatus::Survived)).count();
            let timeout = outcomes.iter().filter(|o| matches!(o.status, MutantStatus::Timeout)).count();
            let error = outcomes.iter().filter(|o| matches!(o.status, MutantStatus::Error)).count();

            let meaningful = killed + survived;
            let mutation_score = if meaningful == 0 {
                None
            } else {
                Some(killed as f64 / meaningful as f64 * 100.0)
            };

            let entry = crap_index.get(&(file.clone(), function.clone())).copied();
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
            let killed = outs.iter().filter(|o| matches!(o.status, MutantStatus::Killed)).count();
            let survived = outs.iter().filter(|o| matches!(o.status, MutantStatus::Survived)).count();
            let timeout = outs.iter().filter(|o| matches!(o.status, MutantStatus::Timeout)).count();
            let error = outs.iter().filter(|o| matches!(o.status, MutantStatus::Error)).count();
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
    operators.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.operator.cmp(&b.operator)));

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
        .map(|s| format!("{:.1}%", s))
        .unwrap_or_else(|| "n/a".to_string());

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
                .map(|v| format!("{:.1}%", v))
                .unwrap_or_else(|| "n/a".into());
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
        let crap = f.crap.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "n/a".into());
        let cc = f.cyclomatic.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into());
        let cov = f.coverage.map(|v| format!("{:.1}%", v)).unwrap_or_else(|| "n/a".into());
        let ms = f.mutation_score.map(|v| format!("{:.1}%", v)).unwrap_or_else(|| "n/a".into());

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
                    "   - {} at line {}: {:?} -> {:?}",
                    s.candidate.operator,
                    s.candidate.line,
                    s.candidate.original,
                    s.candidate.replacement
                );
                let _ = writeln!(
                    out,
                    "     suggestion: {}",
                    s.test_suggestion.prompt
                );
                if let Some(ctx) = &s.source_context {
                    out.push_str("     context:\n");
                    for snippet_line in ctx.snippet.lines() {
                        let _ = writeln!(out, "       {}", snippet_line);
                    }
                }
            }
        }
        out.push('\n');
    }

    out
}
