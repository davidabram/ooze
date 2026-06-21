pub const INIT_CONFIG_TEMPLATE: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze) and .gitignore.
exclude = []

[mutation]
strategy = "actionable"
# operators = ["swap_comparison", "negate_equality", "swap_logical", "swap_boolean"]
# exclude_operators = []
static_skips = true
context_lines = 3
# limit = 50
# lcov = "lcov.info"

[runner]
workspace_backend = "auto"
jobs = 2
timeout_seconds = 120
preflight = true
shared_target = false
warmup = true
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["cargo", "test", "--jobs", "1"]
# env = ["CARGO_HOME=.ooze/cache/cargo-home-{worker}"]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;
