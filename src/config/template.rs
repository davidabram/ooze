pub const LANGUAGES: &[(&str, &str)] = &[
    ("rust", "Rust (cargo test)"),
    ("go", "Go (go test)"),
    ("python", "Python (pytest)"),
    ("node", "Node.js (npm test)"),
    ("java-gradle", "Java / Kotlin (Gradle)"),
    ("java-maven", "Java / Kotlin (Maven)"),
    ("ruby", "Ruby (rake test)"),
];

pub fn template_for_language(lang: &str) -> Option<&'static str> {
    match lang {
        "rust" => Some(RUST),
        "go" => Some(GO),
        "python" => Some(PYTHON),
        "node" => Some(NODE),
        "java-gradle" => Some(JAVA_GRADLE),
        "java-maven" => Some(JAVA_MAVEN),
        "ruby" => Some(RUBY),
        _ => None,
    }
}


const RUST: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze, node_modules, vendor, __pycache__, .gradle) and .gitignore.
exclude = []
# Only mutate files changed versus this git ref (diff BASE...HEAD + uncommitted/untracked).
# changed_only = "main"

[mutation]
strategy = "actionable"
# operators = ["comparison_boundary", "comparison_negation", "negate_equality", "swap_logical", "swap_boolean"]
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
# Each worker gets its own CARGO_TARGET_DIR so parallel builds don't collide.
per_worker_cache = true
warmup = true
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["cargo", "test", "--jobs", "1"]
# {build_cache} resolves to the worker's per_worker_cache dir.
env = ["CARGO_TARGET_DIR={build_cache}"]
# Isolate the cargo registry per worker to avoid fetch races:
# env = ["CARGO_TARGET_DIR={build_cache}", "CARGO_HOME=.ooze/cache/cargo-home-{worker}"]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;

const GO: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze, node_modules, vendor, __pycache__, .gradle) and .gitignore.
exclude = []
# Only mutate files changed versus this git ref (diff BASE...HEAD + uncommitted/untracked).
# changed_only = "main"

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
jobs = 4
timeout_seconds = 60
preflight = true
per_worker_cache = false
warmup = true
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["go", "test", "./..."]
# Give each worker its own build/test cache; disable result caching so every
# mutant re-runs tests rather than returning a stale cached result.
env = ["GOCACHE=.ooze/cache/gocache-{worker}", "GOFLAGS=-count=1"]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;

const PYTHON: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze, node_modules, vendor, __pycache__, .gradle) and .gitignore.
exclude = []
# Only mutate files changed versus this git ref (diff BASE...HEAD + uncommitted/untracked).
# changed_only = "main"

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
jobs = 4
timeout_seconds = 60
preflight = true
per_worker_cache = false
warmup = false
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["pytest", "-q"]
env = [
    "PYTHONDONTWRITEBYTECODE=1",
    "PYTEST_CACHE_DIR=.ooze/cache/pytest-{worker}",
]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;

const NODE: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze, node_modules, vendor, __pycache__, .gradle) and .gitignore.
exclude = []
# Only mutate files changed versus this git ref (diff BASE...HEAD + uncommitted/untracked).
# changed_only = "main"

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
timeout_seconds = 60
preflight = true
per_worker_cache = false
warmup = false
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["npm", "test"]
env = ["npm_config_cache=.ooze/cache/npm-{worker}"]
# For pnpm:  env = ["PNPM_HOME=.ooze/cache/pnpm-{worker}"]
# For yarn:  env = ["YARN_GLOBAL_FOLDER=.ooze/cache/yarn-{worker}"]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;

const JAVA_GRADLE: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze, node_modules, vendor, __pycache__, .gradle) and .gitignore.
exclude = []
# Only mutate files changed versus this git ref (diff BASE...HEAD + uncommitted/untracked).
# changed_only = "main"

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
# JVM startup is slow; give probes more time.
timeout_seconds = 300
preflight = true
per_worker_cache = false
warmup = true
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["./gradlew", "test", "--no-daemon"]
env = ["GRADLE_USER_HOME=.ooze/cache/gradle-{worker}"]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;

const JAVA_MAVEN: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze, node_modules, vendor, __pycache__, .gradle) and .gitignore.
exclude = []
# Only mutate files changed versus this git ref (diff BASE...HEAD + uncommitted/untracked).
# changed_only = "main"

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
timeout_seconds = 300
preflight = true
per_worker_cache = false
warmup = true
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["mvn", "-q", "test"]
env = ["MAVEN_OPTS=-Dmaven.repo.local=.ooze/cache/m2-{worker}"]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;

const RUBY: &str = r#"# ooze config — defaults applied when CLI flags are absent.
# CLI flags always override these values.

[scope]
# Extra exclude globs on top of DEFAULT_EXCLUDES (.git, target, .ooze, node_modules, vendor, __pycache__, .gradle) and .gitignore.
exclude = []
# Only mutate files changed versus this git ref (diff BASE...HEAD + uncommitted/untracked).
# changed_only = "main"

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
timeout_seconds = 60
preflight = true
per_worker_cache = false
warmup = false
cache_dir = ".ooze/cache"
runs_dir = ".ooze/runs"

[probe]
command = ["bundle", "exec", "rake", "test"]
env = ["BUNDLE_PATH=.ooze/cache/bundle-{worker}"]

[report]
format = "human"
# output = "ooze-report.json"
fail_on_survivors = true
allow_incomplete = false
"#;
