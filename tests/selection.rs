use ccrs::{Document, Language, Metric};

#[test]
fn calculates_only_requested_metrics_without_changing_scope_boundaries() {
    let doc = Document::new(
        Language::TypeScript,
        "function f() { if (a && b) { while (c) { run(); } } }".into(),
    )
    .unwrap();
    let all = doc.analyze();
    let selected = doc.analyze_with(&[Metric::Cognitive, Metric::Nesting]);
    assert_eq!(selected.scopes.len(), all.scopes.len());
    for (selected, all) in selected.scopes.iter().zip(&all.scopes) {
        assert_eq!(selected.name, all.name);
        assert_eq!(selected.start_byte, all.start_byte);
        assert_eq!(selected.end_byte, all.end_byte);
        assert_eq!(selected.complexity, None);
        assert_eq!(selected.statement_count, None);
        assert_eq!(selected.max_nesting, all.max_nesting);
        assert_eq!(selected.cognitive_complexity, all.cognitive_complexity);
        let json = serde_json::to_value(selected).unwrap();
        assert!(json.get("complexity").is_none());
        assert!(json.get("statement_count").is_none());
        assert!(json.get("max_nesting").is_some());
    }
}

#[test]
fn every_subset_matches_full_analysis_before_and_after_an_edit() {
    let source = "function outer(x = 0) { if (a && b) { class C {\n\
        [x ? y : z] = a ? b : c;\n\
        static { while (a) { if (b) run(); } }\n\
        method() { return a && b || c && d; }\n\
    } const inner = () => { if (c) { run(); } }; } }";
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        let mut doc = Document::new(language, source.into()).unwrap();
        for edit in [false, true] {
            if edit {
                doc.edit(0, 0, "// 🌲\n").unwrap();
            }
            let all = doc.analyze();
            assert!(!all.has_errors);
            for mask in 0..16 {
                let selected: Vec<_> = Metric::ALL
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, metric)| metric)
                    .collect();
                let mut expected = all.clone();
                for scope in &mut expected.scopes {
                    if !selected.contains(&Metric::Cyclomatic) {
                        scope.complexity = None;
                    }
                    if !selected.contains(&Metric::Nesting) {
                        scope.max_nesting = None;
                    }
                    if !selected.contains(&Metric::Statements) {
                        scope.statement_count = None;
                    }
                    if !selected.contains(&Metric::Cognitive) {
                        scope.cognitive_complexity = None;
                    }
                }
                assert_eq!(
                    doc.analyze_with(&selected),
                    expected,
                    "{language:?} mask={mask}"
                );
            }
            assert_eq!(
                doc.analyze_with(&[Metric::Cognitive, Metric::Cognitive]),
                doc.analyze_with(&[Metric::Cognitive])
            );
            assert_eq!(doc.analyze(), all);
        }
    }
}

#[test]
fn scope_only_analysis_still_reports_incomplete_code() {
    let doc = Document::new(Language::TypeScript, "function f() { if (".into()).unwrap();
    let report = doc.analyze_with(&[]);
    assert!(report.has_errors);
    assert!(!report.scopes.is_empty());
    for scope in &report.scopes {
        assert_eq!(scope.complexity, None);
        assert_eq!(scope.max_nesting, None);
        assert_eq!(scope.statement_count, None);
        assert_eq!(scope.cognitive_complexity, None);
    }
}
