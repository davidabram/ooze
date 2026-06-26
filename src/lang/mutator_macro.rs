//! `mutators!` — declarative builder for a language's `MUTATORS` table.
//!
//! Each entry's `id` (`<language>.<operator>`) and query path are *derived* from
//! the operator variant and query stem, so they can't drift from each other or
//! from the language: the highest-value class of copy-paste mistakes (wrong id
//! prefix, wrong language, wrong query path) becomes unrepresentable. `replace`
//! and `describe` stay arbitrary expressions — a closure or a `fn` path — since
//! those genuinely vary per operator.
//!
//! ```ignore
//! mutators! {
//!     language: Rust,
//!     id_prefix: "rust",
//!     SwapBoolean {
//!         replace: |o| match o { "true" => Some("false".into()), _ => None },
//!         describe: |o, r| format!("Swap boolean literal {o} -> {r}"),
//!     },
//! }
//! ```
//!
//! expands to `id: "rust.swap_boolean"`, `operator: OperatorName::SwapBoolean`,
//! `language: Language::Rust`, and `query:
//! include_str!(".../queries/rust/swap_boolean.scm")`. The operator's snake form
//! (`swap_boolean`) is the single source for both the id suffix and the query
//! filename, so they can't drift — the `.scm` files are named to match.
//!
//! `id_prefix` is the language's canonical string (== `Language::as_str` == the
//! `queries/<dir>` name). It's passed rather than derived from the variant
//! because case-splitting `JavaScript` yields `java_script`, not `javascript`;
//! the registry test still asserts `id == "<prefix>.<operator>"`, so a wrong
//! prefix can't slip through.

macro_rules! mutators {
    (
        language: $lang:ident,
        id_prefix: $prefix:literal,
        $(
            $op:ident {
                replace: $replace:expr,
                describe: $describe:expr
                $(, default_enabled: $de:expr)?
                $(,)?
            }
        ),* $(,)?
    ) => {
        pub const MUTATORS: &[$crate::core::MutatorImpl] = &[
            $(
                ::paste::paste! {
                    $crate::core::MutatorImpl {
                        id: concat!(
                            $prefix,
                            ".",
                            stringify!([<$op:snake>])
                        ),
                        operator: $crate::core::OperatorName::$op,
                        language: $crate::core::Language::$lang,
                        // Absolute path via CARGO_MANIFEST_DIR so resolution
                        // doesn't depend on which source file expands the macro.
                        // Filename is the operator's snake form, matching the id.
                        query: include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/queries/",
                            $prefix,
                            "/",
                            stringify!([<$op:snake>]),
                            ".scm"
                        )),
                        replacement: $replace,
                        description: $describe,
                        default_enabled_override: mutators!(@default $($de)?),
                    }
                }
            ),*
        ];
    };

    // Optional per-language default-enabled override; absent means inherit the
    // operator-level default (`None`).
    (@default) => { None };
    (@default $de:expr) => { Some($de) };
}

pub(crate) use mutators;
