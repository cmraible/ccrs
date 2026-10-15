use ccrs::{Document, Language};

#[test]
fn counts_decisions_within_a_function() {
    let doc = Document::new(
        Language::JavaScript,
        "function choose(x) { if (x) return 1; return 0; }".into(),
    )
    .unwrap();
    let report = doc.analyze();
    assert!(!report.has_errors);
    assert_eq!(report.scopes.len(), 2);
    assert_eq!(report.scopes[0].complexity, 1);
    assert_eq!(report.scopes[1].name, "choose");
    assert_eq!(report.scopes[1].complexity, 2);
}

#[test]
fn counts_each_runtime_branch_but_not_nonbranching_syntax() {
    for (body, decisions) in [
        ("if (a) {} else if (b) {} else {}", 2),
        ("for (;;) {} while (a) {} do {} while (a);", 3),
        ("for (const x of xs) {} for (const k in obj) {}", 2),
        ("try {} catch (e) {} finally {}", 1),
        (
            "switch(x) { case 1: break; case 2: break; default: break; }",
            2,
        ),
        ("return a ? b : c;", 1),
        ("return (a && b || c) ?? d;", 3),
        ("a &&= b; a ||= b; a ??= b;", 3),
        ("return a?.b?.[c]?.();", 3),
        ("const {a = 1} = x; const [b = 2] = y;", 2),
        (
            "const s = 'if && ?'; /* while || */ return /a?b/.test(s);",
            0,
        ),
        ("a += 1; return a & b | c;", 0),
    ] {
        for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
            let source = format!("function f(a, b, c, d) {{ {body} }}");
            let report = Document::new(language, source).unwrap().analyze();
            assert!(!report.has_errors, "{language:?}: {body}");
            assert_eq!(
                report.scopes[1].complexity,
                1 + decisions,
                "{language:?}: {body}"
            );
        }
    }
}

#[test]
fn isolates_nested_functions_and_default_parameters() {
    let source = "function outer(x = 0) {\n\
        const arrow = (a = 1) => a ? 1 : 0;\n\
        const expr = function (b = 2) { return b && x; };\n\
        function* gen() { if (x) yield x; }\n\
        const obj = { method() { return x || 1; } };\n\
        const g = function* () { while (x) yield x; };\n\
        return arrow(x);\n\
    }";
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let report = Document::new(language, source.into()).unwrap().analyze();
        assert!(!report.has_errors);
        let scores: Vec<_> = report
            .scopes
            .iter()
            .map(|s| (s.name.as_str(), s.complexity))
            .collect();
        assert_eq!(
            scores,
            [
                ("<module>", 1),
                ("outer", 2),
                ("arrow", 3),
                ("expr", 3),
                ("gen", 2),
                ("method", 2),
                ("g", 2)
            ],
            "{language:?}"
        );
    }
}

#[test]
fn class_initializers_are_independent_but_computed_names_run_outside_methods() {
    let source = "function outer() { class C {\n\
        [a || b] = c && d;\n\
        field = e ? f : g;\n\
        static { if (x) y(); }\n\
        [h ?? i]() { if (j) k(); }\n\
    } }";
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let report = Document::new(language, source.into()).unwrap().analyze();
        assert!(!report.has_errors);
        let scores: Vec<_> = report.scopes.iter().map(|s| s.complexity).collect();
        assert_eq!(scores, [1, 3, 2, 2, 2, 2], "{language:?}");
    }
}

#[test]
fn edits_match_fresh_analysis_including_unicode_and_incomplete_code() {
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let mut source = "const label = '🌲';\nfunction f(x) { return x; }".to_string();
        let mut doc = Document::new(language, source.clone()).unwrap();
        let start = source.find("return x;").unwrap();
        for replacement in ["if (x) {", "return x?.y ?? 0;", "return 'é';\n", ""] {
            let end = source.rfind('}').unwrap() - 1;
            doc.edit(start, end, replacement).unwrap();
            source.replace_range(start..end, replacement);
            assert_eq!(
                doc.analyze(),
                Document::new(language, source.clone()).unwrap().analyze()
            );
        }
    }
}

#[test]
fn rejects_bad_edits_without_changing_the_document() {
    let mut doc = Document::new(Language::JavaScript, "const s = '🌲';".into()).unwrap();
    let before = doc.analyze();
    for (start, end) in [(12, 12), (99, 99), (5, 2), (0, usize::MAX)] {
        assert!(doc.edit(start, end, "x").is_err());
        assert_eq!(doc.analyze(), before);
    }
}

#[test]
fn reports_parse_errors_and_recovers_after_an_edit() {
    let mut doc = Document::new(Language::TypeScript, "function f() { if (".into()).unwrap();
    assert!(doc.analyze().has_errors);
    doc.edit(19, 19, "true) {} }").unwrap();
    let report = doc.analyze();
    assert!(!report.has_errors);
    assert_eq!(report.scopes[1].complexity, 2);
}

#[test]
fn types_and_jsx_text_do_not_count_as_runtime_branches() {
    let source = "type Result<T> = T extends string ? true : false;\n\
        interface I { f(x?: number): void; }\n\
        const View = (x: number = 0) => <div>if &amp;&amp; ? {x ? 'a' : 'b'}</div>;";
    let report = Document::new(Language::Tsx, source.into())
        .unwrap()
        .analyze();
    assert!(!report.has_errors);
    assert_eq!(report.scopes.len(), 2);
    assert_eq!(report.scopes[0].complexity, 1);
    assert_eq!(report.scopes[1].complexity, 3);
}

#[test]
fn insertions_and_deletions_at_every_utf8_boundary_match_a_fresh_parse() {
    let source = "// 🌲\r\nconst f = (x = 'é') => x?.y ?? 0;\r\nclass C { f = () => x && y; }\n";
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let boundaries: Vec<_> = source
            .char_indices()
            .map(|(i, _)| i)
            .chain([source.len()])
            .collect();
        let mut doc = Document::new(language, source.into()).unwrap();
        for &start in &boundaries {
            for text in ["?", "\n🌲\n", ""] {
                let mut changed = source.to_owned();
                changed.insert_str(start, text);
                doc.edit(start, start, text).unwrap();
                assert_eq!(
                    doc.analyze(),
                    Document::new(language, changed).unwrap().analyze(),
                    "{language:?} insertion at {start}"
                );
                doc.edit(start, start + text.len(), "").unwrap();
            }
        }
        for pair in boundaries.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let mut changed = source.to_owned();
            changed.replace_range(start..end, "");
            doc.edit(start, end, "").unwrap();
            assert_eq!(
                doc.analyze(),
                Document::new(language, changed).unwrap().analyze(),
                "{language:?} deletion at {start}"
            );
            doc.edit(start, start, &source[start..end]).unwrap();
        }
        doc.edit(0, source.len(), "").unwrap();
        assert_eq!(
            doc.analyze(),
            Document::new(language, String::new()).unwrap().analyze()
        );
    }
}
