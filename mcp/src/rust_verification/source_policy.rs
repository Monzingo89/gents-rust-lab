//! Narrow pure-function dialect. The OS sandbox remains a separate boundary.
use anyhow::{ensure, Context, Result};
use syn::{
    parse::Parser,
    visit::{self, Visit},
    Item,
};
struct Policy {
    tests: bool,
    valid: bool,
    reasons: std::collections::BTreeSet<String>,
}
impl Policy {
    fn reject(&mut self, reason: impl Into<String>) {
        self.valid = false;
        if self.reasons.len() < 12 {
            self.reasons
                .insert(reason.into().chars().take(240).collect());
        }
    }
}
impl<'ast> Visit<'ast> for Policy {
    fn visit_item(&mut self, item: &'ast Item) {
        if let Item::Fn(function) = item {
            if matches!(function.sig.safety, syn::Safety::Unsafe(_))
                || function.sig.abi.is_some()
                || function.sig.asyncness.is_some()
            {
                self.reject("unsafe, extern or async function declaration");
            }
            visit::visit_item_fn(self, function);
        } else {
            self.reject(
                "non-function item (imports, modules, structs and macro definitions are excluded)",
            );
        }
    }
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if !self.tests || !attr.path().is_ident("test") || !matches!(attr.meta, syn::Meta::Path(_))
        {
            self.reject("attribute outside owner-frozen #[test]");
        }
    }
    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments = path
            .segments
            .iter()
            .map(|p| p.ident.to_string())
            .collect::<Vec<_>>();
        if let Some(root) = segments.first() {
            if ["std", "core", "alloc"].contains(&root.as_str())
                && !segments.get(1).is_some_and(|s| {
                    [
                        "cmp",
                        "collections",
                        "iter",
                        "mem",
                        "option",
                        "result",
                        "str",
                        "string",
                        "vec",
                        "num",
                        "convert",
                        "ops",
                    ]
                    .contains(&s.as_str())
                })
            {
                self.reject(format!(
                    "forbidden standard-library module: {}::{}",
                    root,
                    segments.get(1).map_or("", String::as_str)
                ));
            }
        }
        visit::visit_path(self, path);
    }
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        // Candidate macros are forbidden, including include!/env!/user macros.
        // Only frozen acceptance assertions are allowed; inspect their arguments.
        let name = mac
            .path
            .segments
            .iter()
            .map(|p| p.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        if !self.tests
            || ![
                "assert",
                "assert_eq",
                "assert_ne",
                "std::assert",
                "std::assert_eq",
                "std::assert_ne",
            ]
            .contains(&name.as_str())
        {
            self.reject(if self.tests {
                format!("only assertion macros allowed in frozen acceptance; rejected macro: {name}!")
            } else {
                let help = if matches!(name.as_str(), "vec" | "std::vec") {
                    "macro-free APIs include Vec::new() followed by .resize(length, value), Vec::from(array), or iterator .collect()"
                } else { "use macro-free calls in permitted standard-library modules" };
                format!("candidate macro invocation forbidden: {name}!; {help}")
            });
            return;
        }
        let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        match parser.parse2(mac.tokens.clone()) {
            Ok(args) => {
                for expr in args {
                    self.visit_expr(&expr);
                }
            }
            Err(_) => self.reject("acceptance assertion arguments must be parseable expressions"),
        }
    }
    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if matches!(expr, syn::Expr::Verbatim(_)) {
            self.reject("unsupported verbatim expression");
        } else {
            visit::visit_expr(self, expr);
        }
    }
    fn visit_type(&mut self, ty: &'ast syn::Type) {
        if matches!(ty, syn::Type::Verbatim(_)) {
            self.reject("unsupported verbatim type");
        } else {
            visit::visit_type(self, ty);
        }
    }
    fn visit_expr_unsafe(&mut self, _: &'ast syn::ExprUnsafe) {
        self.reject("unsafe expression");
    }
    fn visit_expr_async(&mut self, _: &'ast syn::ExprAsync) {
        self.reject("async expression");
    }
}
pub fn validate(text: &str, tests: bool) -> Result<usize> {
    ensure!(
        !text.is_empty() && text.len() <= 8192,
        "Rust source must contain 1..8192 UTF-8 bytes"
    );
    // Default display stays generic (including confidential acceptance errors).
    // Candidate preflight may render the bounded cause chain explicitly.
    let parsed = syn::parse_file(text).context("invalid Rust syntax")?;
    ensure!(
        parsed.shebang.is_none() && parsed.attrs.is_empty() && !parsed.items.is_empty(),
        "Rust verifier requires function items without crate attributes"
    );
    let mut policy = Policy {
        tests,
        valid: true,
        reasons: Default::default(),
    };
    policy.visit_file(&parsed);
    ensure!(policy.valid,"Rust source policy rejected: {}. Only safe pure function items and permitted std/core modules; assertion macros only in frozen acceptance.",policy.reasons.into_iter().collect::<Vec<_>>().join("; "));
    let count = parsed
        .items
        .iter()
        .filter(
            |item| matches!(item,Item::Fn(f) if f.attrs.iter().any(|a|a.path().is_ident("test"))),
        )
        .count();
    ensure!(
        !tests || (1..=16).contains(&count),
        "frozen acceptance requires 1..16 test functions"
    );
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejection_names_the_construct_without_supplying_candidate_code() {
        let error = validate("pub fn f()->Vec<u32>{vec![1,2]}", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("candidate macro invocation forbidden: vec!"));
        assert!(error.contains(".resize(length, value)"));
        assert!(!error.contains("vec![1,2]"));
        let error = validate("pub fn f(){let _=std::fs::read(\"x\");}", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("forbidden standard-library module: std::fs"));
    }
    #[test]
    fn rejects_code_execution_and_macro_or_alias_escape_routes() {
        for source in [
            "pub fn f(){std::process::exit(0)}",
            "use std::process as p; pub fn f(){p::exit(0)}",
            "pub fn f(){let _=include_str!(\"/etc/passwd\");}",
            "pub fn f(){let _=env!(\"HOME\");}",
            "extern crate std as x;pub fn f(){x::process::exit(0)}",
            "#[path=\"/tmp/x\"]mod x;",
            "pub fn f(){unsafe{}}",
            "pub fn f(){macro_rules! x{()=>{}};}",
            "#[unsafe(no_mangle)] pub fn f(){}",
            "pub fn f(){std::thread::spawn(||{});}",
            "pub fn f(){let _=std::fs::read(\"x\");}",
        ] {
            assert!(validate(source, false).is_err(), "{source}");
        }
        assert!(validate("#[test]fn t(){assert_eq!(std::process::exit(0),());}", true).is_err());
    }
    #[test]
    fn accepts_pure_helpers_and_trusted_assertions() {
        validate(
            "pub fn f(x:&str)->usize{x.chars().count()} fn helper()->Option<usize>{Some(2)}",
            false,
        )
        .unwrap();
        assert_eq!(
            validate(
                "#[test] fn t(){std::assert_eq!(candidate::f(\"α\"),1);}",
                true
            )
            .unwrap(),
            1
        );
    }
}
