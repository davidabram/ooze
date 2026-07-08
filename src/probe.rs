//! Typed probe command: a non-empty program-plus-args pair. Constructed once
//! during CLI/config resolution, so runner code never sees the "empty probe
//! command" invalid state and never has to re-check it defensively.

/// A probe command with its program split from its arguments. Non-empty by
/// construction: [`ProbeCommand::new`] rejects an empty parts list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct ProbeCommand {
    program: String,
    args: Vec<String>,
}

impl ProbeCommand {
    /// Split `parts` into program and args; errors when `parts` is empty.
    pub(crate) fn new(parts: Vec<String>) -> anyhow::Result<Self> {
        let mut parts = parts.into_iter();
        let Some(program) = parts.next() else {
            anyhow::bail!("probe command is empty");
        };
        Ok(Self {
            program,
            args: parts.collect(),
        })
    }

    /// Build from a hardcoded non-empty command, e.g. a preset default.
    /// Panics on an empty slice, which is a bug in the caller's literal.
    pub(crate) fn from_static(parts: &[&str]) -> Self {
        Self::new(parts.iter().map(ToString::to_string).collect())
            .expect("static probe command must be non-empty")
    }

    pub(crate) fn program(&self) -> &str {
        &self.program
    }

    pub(crate) fn args(&self) -> &[String] {
        &self.args
    }

    /// The original parts, program first.
    pub(crate) fn as_vec(&self) -> Vec<String> {
        std::iter::once(self.program.clone())
            .chain(self.args.iter().cloned())
            .collect()
    }

    /// Space-joined command for logs and error messages. No shell quoting.
    pub(crate) fn display(&self) -> String {
        self.as_vec().join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_command() {
        let err = ProbeCommand::new(vec![]).unwrap_err();
        assert!(err.to_string().contains("probe command is empty"));
    }

    #[test]
    fn splits_program_from_args() {
        let probe = ProbeCommand::new(vec!["cargo".into(), "test".into(), "--lib".into()]).unwrap();
        assert_eq!(probe.program(), "cargo");
        assert_eq!(probe.args(), ["test", "--lib"]);
    }

    #[test]
    fn from_static_builds_the_same_command() {
        assert_eq!(
            ProbeCommand::from_static(&["cargo", "test"]),
            ProbeCommand::new(vec!["cargo".into(), "test".into()]).unwrap()
        );
    }

    #[test]
    fn display_joins_with_spaces() {
        assert_eq!(
            ProbeCommand::from_static(&["cargo", "test"]).display(),
            "cargo test"
        );
    }

    #[test]
    fn as_vec_round_trips() {
        let parts = vec!["go".to_string(), "test".to_string(), "./...".to_string()];
        assert_eq!(ProbeCommand::new(parts.clone()).unwrap().as_vec(), parts);
    }
}
