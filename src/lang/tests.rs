use super::scan_directory;
use crate::core::FunctionSpan;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

fn spans() -> &'static [FunctionSpan] {
    static SPANS: OnceLock<Vec<FunctionSpan>> = OnceLock::new();
    SPANS.get_or_init(|| {
        scan_directory(Path::new("tests/fixtures/lang")).expect("scanning fixtures")
    })
}

fn named_complexity() -> HashMap<(String, String), usize> {
    spans()
        .iter()
        .filter(|s| !s.name.starts_with("<anonymous>"))
        .map(|s| {
            let file = s
                .file
                .file_name()
                .and_then(|n| n.to_str())
                .expect("file name")
                .to_string();
            ((file, s.name.clone()), s.cyclomatic)
        })
        .collect()
}

fn anonymous_complexity(file: &str) -> Vec<usize> {
    spans()
        .iter()
        .filter(|s| {
            s.file.file_name().and_then(|n| n.to_str()) == Some(file)
                && s.name.starts_with("<anonymous>")
        })
        .map(|s| s.cyclomatic)
        .collect()
}

#[test]
fn cyclomatic_values_match_expected() {
    let map = named_complexity();

    // (file, function name) -> expected cyclomatic complexity
    let expected: &[(&str, &str, usize)] = &[
        // rust
        ("sample.rs", "plain", 1),
        ("sample.rs", "if_else", 2),
        ("sample.rs", "if_elif_else", 3),
        ("sample.rs", "loops_and_match", 6),
        ("sample.rs", "bool_ops", 3),
        ("sample.rs", "try_op", 2),
        ("sample.rs", "let_chain_opt", 3),
        ("sample.rs", "outer_with_closure", 1),
        // python
        ("sample.py", "plain", 1),
        ("sample.py", "if_elif_else", 3),
        ("sample.py", "loops", 3),
        ("sample.py", "bool_ops", 3),
        ("sample.py", "ternary", 2),
        ("sample.py", "comprehension", 2),
        ("sample.py", "match_demo", 4),
        ("sample.py", "nested", 1),
        ("sample.py", "inner", 2),
        // ruby
        ("sample.rb", "plain", 1),
        ("sample.rb", "if_elsif", 3),
        ("sample.rb", "unless_demo", 2),
        ("sample.rb", "case_when", 3),
        ("sample.rb", "loops", 4),
        ("sample.rb", "postfix", 2),
        ("sample.rb", "bool_ops", 3),
        ("sample.rb", "ternary", 2),
        ("sample.rb", "pattern_match", 3),
        // c
        ("sample.c", "plain", 1),
        ("sample.c", "if_else", 3),
        ("sample.c", "loops", 4),
        ("sample.c", "switch_case", 4),
        ("sample.c", "ternary", 2),
        ("sample.c", "bool_ops", 3),
        ("sample.c", "preproc_inside", 4),
        // cpp
        ("sample.cpp", "plain", 1),
        ("sample.cpp", "if_else", 3),
        ("sample.cpp", "loops", 5),
        ("sample.cpp", "switch_case", 4),
        ("sample.cpp", "ternary", 2),
        ("sample.cpp", "bool_ops", 3),
        ("sample.cpp", "try_catch", 3),
        ("sample.cpp", "preproc_inside", 4),
        ("sample.cpp", "with_lambda", 1),
        // c_sharp
        ("sample.cs", "Plain", 1),
        ("sample.cs", "IfElse", 3),
        ("sample.cs", "Loops", 5),
        ("sample.cs", "SwitchCase", 4),
        ("sample.cs", "SwitchExpr", 4),
        ("sample.cs", "Ternary", 2),
        ("sample.cs", "BoolOps", 3),
        ("sample.cs", "NullCoalesce", 2),
        ("sample.cs", "TryCatch", 3),
        ("sample.cs", "CatchFilter", 3),
        ("sample.cs", "CaseGuard", 4),
        ("sample.cs", "WithLambda", 1),
        // javascript
        ("sample.js", "plain", 1),
        ("sample.js", "ifElse", 3),
        ("sample.js", "loops", 4),
        ("sample.js", "switchCase", 4),
        ("sample.js", "ternary", 2),
        ("sample.js", "boolOps", 3),
        ("sample.js", "nullCoalesce", 2),
        ("sample.js", "tryCatch", 3),
        // typescript
        ("sample.ts", "plain", 1),
        ("sample.ts", "ifElse", 3),
        ("sample.ts", "loops", 4),
        ("sample.ts", "switchCase", 4),
        ("sample.ts", "ternary", 2),
        ("sample.ts", "boolOps", 3),
        ("sample.ts", "nullCoalesce", 2),
        ("sample.ts", "tryCatch", 3),
        // go
        ("sample.go", "plain", 1),
        ("sample.go", "ifElse", 3),
        ("sample.go", "loops", 3),
        ("sample.go", "switchCase", 4),
        ("sample.go", "typeSwitch", 4),
        ("sample.go", "boolOps", 3),
        ("sample.go", "withClosure", 1),
        // java
        ("sample.java", "plain", 1),
        ("sample.java", "ifElse", 3),
        ("sample.java", "loops", 5),
        ("sample.java", "switchCase", 4),
        ("sample.java", "switchExpr", 4),
        ("sample.java", "ternary", 2),
        ("sample.java", "boolOps", 3),
        ("sample.java", "tryCatch", 3),
        ("sample.java", "withLambda", 1),
        // php
        ("sample.php", "plain", 1),
        ("sample.php", "ifElse", 3),
        ("sample.php", "loops", 5),
        ("sample.php", "switchCase", 4),
        ("sample.php", "matchExpr", 4),
        ("sample.php", "ternary", 2),
        ("sample.php", "boolOps", 3),
        ("sample.php", "nullCoalesce", 2),
        ("sample.php", "tryCatch", 3),
        ("sample.php", "withClosure", 1),
    ];

    let mut missing = Vec::new();
    let mut wrong = Vec::new();
    for (file, name, want) in expected {
        match map.get(&(file.to_string(), name.to_string())) {
            None => missing.push(format!("{file}:{name} (expected {want})")),
            Some(got) if *got != *want => {
                wrong.push(format!("{file}:{name} = {got}, expected {want}"))
            }
            _ => {}
        }
    }
    let extra: Vec<String> = map
        .iter()
        .filter(|((file, name), _)| {
            !expected
                .iter()
                .any(|(f, n, _)| *f == file.as_str() && *n == name.as_str())
        })
        .map(|((file, name), c)| format!("{file}:{name} = {c} (unexpected)"))
        .collect();

    let mut report = String::new();
    if !missing.is_empty() {
        report.push_str("missing functions:\n  ");
        report.push_str(&missing.join("\n  "));
        report.push('\n');
    }
    if !wrong.is_empty() {
        report.push_str("wrong complexity:\n  ");
        report.push_str(&wrong.join("\n  "));
        report.push('\n');
    }
    if !extra.is_empty() {
        report.push_str("unexpected functions:\n  ");
        report.push_str(&extra.join("\n  "));
        report.push('\n');
    }
    assert!(report.is_empty(), "cyclometric scan mismatches:\n{report}");
}

#[test]
fn anonymous_functions_get_synthetic_names_and_correct_complexity() {
    for file in [
        "sample.rs",
        "sample.cpp",
        "sample.cs",
        "sample.js",
        "sample.ts",
        "sample.go",
        "sample.java",
        "sample.php",
    ] {
        let anon = anonymous_complexity(file);
        assert_eq!(
            anon,
            [2],
            "{file}: expected one anonymous function with cyclomatic 2, got {anon:?}"
        );
    }
}
