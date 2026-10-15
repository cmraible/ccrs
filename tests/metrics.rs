use ccrs::{Document, Language};

#[test]
fn nesting_tracks_control_flow_and_resets_for_nested_scopes() {
    let source = "function flat() { if (a) {} if (b) {} if (c) {} }\n\
        function nested() { if (a) { while (b) { if (c) {} } } }\n\
        function outer() { if (a) { const inner = () => { if (b) {} }; } }";
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let report = Document::new(language, source.into()).unwrap().analyze();
        assert!(!report.has_errors);
        let depths: Vec<_> = report
            .scopes
            .iter()
            .map(|s| (s.name.as_str(), s.max_nesting))
            .collect();
        assert_eq!(
            depths,
            [
                ("<module>", 0),
                ("flat", 1),
                ("nested", 3),
                ("outer", 1),
                ("inner", 1)
            ]
        );
    }
}

#[test]
fn statement_count_follows_eslint_block_entries_and_excludes_nested_bodies() {
    for (body, expected) in [
        ("", 0),
        ("// comment\n/* another */", 0),
        ("const a = 1, b = 2; return a + b;", 2),
        ("if (a) { run(); } else { stop(); }", 3),
        ("if (a) run();", 1),
        ("for (let i = 0; i < 3; i++) { run(); }", 2),
        ("switch (x) { case 1: run(); break; default: stop(); }", 1),
        ("{ run(); } ;", 3),
        (
            "function inner() { run(); stop(); } const arrow = () => { run(); };",
            2,
        ),
        (
            "class C { static { run(); stop(); } method() { run(); } }",
            1,
        ),
    ] {
        for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
            let report = Document::new(language, format!("function f() {{ {body} }}"))
                .unwrap()
                .analyze();
            assert!(!report.has_errors, "{body}");
            assert_eq!(
                report.scopes[1].statement_count, expected,
                "{language:?}: {body}"
            );
            assert_eq!(report.scopes[0].statement_count, 1);
        }
    }
}

#[test]
fn nesting_ignores_plain_blocks_and_else_if_but_counts_try_and_switch() {
    for (body, expected) in [
        ("{{{ run(); }}}", 0),
        ("if (a) {} else if (b) {} else { if (c) {} }", 2),
        ("if (a) {} else if (b) { if (c) {} }", 2),
        (
            "try { if (a) {} } catch (e) { while (b) {} } finally { if (c) {} }",
            2,
        ),
        ("switch (x) { case 1: if (a) { while (b) {} } }", 3),
        ("return a ? (b ? c : d) : e;", 0),
        ("for (;;) { for (const x of xs) { do {} while (a); } }", 3),
    ] {
        for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
            let report = Document::new(language, format!("function f() {{ {body} }}"))
                .unwrap()
                .analyze();
            assert!(!report.has_errors, "{body}");
            assert_eq!(
                report.scopes[1].max_nesting, expected,
                "{language:?}: {body}"
            );
        }
    }
}

#[test]
fn cognitive_complexity_penalizes_nesting_and_starts_at_zero() {
    for (body, expected) in [
        ("return 0;", 0),
        ("if (a) {} if (b) {} if (c) {}", 3),
        ("if (a) { if (b) { if (c) {} } }", 6),
        ("if (a) {} else if (b) {} else {}", 3),
        ("while (a) { if (b) {} else if (c) {} else {} }", 5),
        (
            "try { run(); } catch (e) { if (a) {} } finally { if (b) {} }",
            4,
        ),
        (
            "switch (x) { case 1: if (a) {} break; case 2: break; default: break; }",
            3,
        ),
        ("return a ? (b ? c : d) : e;", 3),
        ("return (a ? b : c) ? d : e;", 2),
        ("outer: while (a) { if (b) break outer; continue; }", 4),
        ("for (;;) { for (const x of xs) { do {} while (a); } }", 6),
    ] {
        for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
            let report = Document::new(language, format!("function f() {{ {body} }}"))
                .unwrap()
                .analyze();
            assert!(!report.has_errors, "{body}");
            assert_eq!(
                report.scopes[1].cognitive_complexity, expected,
                "{language:?}: {body}"
            );
        }
    }
}

#[test]
fn cognitive_logical_sequences_follow_sonarjs_operator_rules() {
    for (expression, expected) in [
        ("a && b && c", 1),
        ("a || b || c", 0),
        ("a ?? b", 0),
        ("a && b || c && d", 2),
        ("a && (b || c) && d", 2),
        ("a && (b && c)", 1),
        ("a && call(b && c)", 2),
        ("a?.b?.() ?? c", 0),
        ("a &&= b", 0),
        ("a ||= b", 0),
        ("a ??= b", 0),
        ("f(a)", 0),
    ] {
        for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
            let report = Document::new(
                language,
                format!("function f(a = 0) {{ return {expression}; }}"),
            )
            .unwrap()
            .analyze();
            assert!(!report.has_errors, "{expression}");
            assert_eq!(
                report.scopes[1].cognitive_complexity, expected,
                "{language:?}: {expression}"
            );
        }
    }
}

#[test]
fn cognitive_complexity_recognizes_jsx_short_circuit_idioms() {
    for (expression, expected) in [
        ("a && <Item />", 0),
        ("a && b && <Item />", 0),
        ("(a && b)", 0),
        ("a && b || c && <Item />", 2),
        ("a && (b ? <A /> : <B />)", 2),
        ("a && call(b && c)", 1),
    ] {
        for language in [Language::JavaScript, Language::Tsx] {
            let report = Document::new(
                language,
                format!("const View = () => <div>{{ {expression} }}</div>;"),
            )
            .unwrap()
            .analyze();
            assert!(!report.has_errors, "{expression}");
            assert_eq!(
                report.scopes[1].cognitive_complexity, expected,
                "{language:?}: {expression}"
            );
        }
    }
}

#[test]
fn cognitive_sequences_respect_typescript_assertion_precedence() {
    for (expression, expected) in [
        ("(a && b as boolean) && c", 1),
        ("(a && b!) && c", 1),
        ("((a && b)!) && c", 2),
        ("((a && b) as boolean) && c", 2),
        ("(a && b satisfies boolean) && c", 1),
        ("((a && b) satisfies boolean) && c", 2),
        ("(a && b as unknown as boolean) && c", 1),
        ("((a && b) as unknown as boolean) && c", 2),
    ] {
        for language in [Language::TypeScript, Language::Tsx] {
            let report = Document::new(language, format!("const f = () => {expression};"))
                .unwrap()
                .analyze();
            assert!(!report.has_errors);
            assert_eq!(
                report.scopes[1].cognitive_complexity, expected,
                "{expression}"
            );
        }
    }
}

#[test]
fn all_metrics_keep_class_and_function_scopes_independent() {
    let source = "function outer() { if (a) { class C {\n\
        [b ? c : d] = x ? y : z;\n\
        field = () => { if (f) return 1; };\n\
        static { while (g) { if (h) run(); } }\n\
        [i ? j : k]() { if (l) { if (m) run(); } }\n\
    } const inner = () => { if (n) return 1; }; } }";
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let report = Document::new(language, source.into()).unwrap().analyze();
        assert!(!report.has_errors);
        let metrics: Vec<_> = report
            .scopes
            .iter()
            .map(|s| (s.max_nesting, s.statement_count, s.cognitive_complexity))
            .collect();
        assert_eq!(
            metrics,
            [
                (0, 1, 0),
                (1, 3, 5),
                (0, 0, 1),
                (0, 0, 0),
                (1, 1, 1),
                (2, 2, 3),
                (2, 2, 3),
                (1, 1, 1)
            ],
            "{language:?}"
        );
    }
}

#[test]
fn flattening_an_edited_function_reduces_nesting_and_cognitive_complexity() {
    let nested = "function f() { if (a) { if (b) { if (c) {} } } }";
    let flat = "function f() { if (a) {} if (b) {} if (c) {} }";
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let mut document = Document::new(language, nested.into()).unwrap();
        let before = document.analyze();
        document.edit(0, nested.len(), flat).unwrap();
        let after = document.analyze();
        assert_eq!(
            after,
            Document::new(language, flat.into()).unwrap().analyze()
        );
        assert_eq!(
            (before.scopes[1].complexity, after.scopes[1].complexity),
            (4, 4)
        );
        assert_eq!(
            (before.scopes[1].max_nesting, after.scopes[1].max_nesting),
            (3, 1)
        );
        assert_eq!(
            (
                before.scopes[1].cognitive_complexity,
                after.scopes[1].cognitive_complexity
            ),
            (6, 3)
        );
        assert_eq!(
            (
                before.scopes[1].statement_count,
                after.scopes[1].statement_count
            ),
            (3, 3)
        );
    }
}
