use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ccrs"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn cli_analyzes_unsaved_typescript_and_enforces_a_threshold() {
    let output = run(
        &["--json", "--max", "1", "--stdin", "example.ts"],
        "function f(x: boolean) { if (x) return 1; return 0; }",
    );
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["path"], "example.ts");
    assert_eq!(result["report"]["scopes"][1]["complexity"], 2);
}

#[test]
fn service_updates_buffers_and_rejects_stale_edits_without_losing_state() {
    let requests = [
        serde_json::json!({"op":"open", "path":"a.ts", "source":"const f = () => 1;"}),
        serde_json::json!({"op":"edit", "path":"a.ts", "version":0, "start_byte":16, "end_byte":17, "text":"x ? 1 : 0"}),
        serde_json::json!({"op":"edit", "path":"a.ts", "version":0, "start_byte":0, "end_byte":0, "text":"?"}),
        serde_json::json!({"op":"edit", "path":"a.ts", "version":1, "start_byte":0, "end_byte":0, "text":"\n"}),
        serde_json::json!({"op":"close", "path":"a.ts"}),
        serde_json::json!({"op":"edit", "path":"a.ts", "version":2, "start_byte":0, "end_byte":0, "text":""}),
    ];
    let input = requests
        .iter()
        .map(|r| format!("{r}\n"))
        .collect::<String>();
    let output = run(&["serve"], &input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let results: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(results.len(), 6);
    assert_eq!(results[0]["version"], 0);
    assert_eq!(results[1]["version"], 1);
    assert_eq!(results[1]["report"]["scopes"][1]["complexity"], 2);
    assert!(results[2]["error"].as_str().unwrap().contains("version"));
    assert_eq!(results[3]["version"], 2);
    assert_eq!(results[3]["report"]["scopes"][1]["line"], 2);
    assert_eq!(results[4]["closed"], true);
    assert!(results[5]["error"].is_string());
}

#[test]
fn malformed_requests_do_not_stop_the_service() {
    let output = run(
        &["serve"],
        "not json\n{\"op\":\"open\",\"path\":\"a.js\",\"source\":\"\"}\n",
    );
    assert!(output.status.success());
    let results: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(results[0]["error"].is_string());
    assert_eq!(results[1]["report"]["scopes"][0]["complexity"], 1);
}

#[test]
fn human_output_and_parse_error_exit_code() {
    let output = run(&["--stdin", "input.js"], "function f() { if (");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("input.js:")
    );
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("provisional")
    );
}

#[test]
fn service_flushes_each_response_before_stdin_closes() {
    use std::io::{BufRead, BufReader};
    let mut child = Command::new(env!("CARGO_BIN_EXE_ccrs"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    writeln!(
        input,
        "{{\"op\":\"open\",\"path\":\"a.js\",\"source\":\"\"}}"
    )
    .unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["version"],
        0
    );
    drop(input);
    assert!(child.wait().unwrap().success());
}

#[test]
fn cli_displays_and_enforces_each_metric_independently() {
    let source = "function f() { if (a) { if (b) { run(); } } }";
    let output = run(&["--stdin", "a.js"], source);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("cyclomatic=3 nesting=2 statements=3 cognitive=3"),
        "{text}"
    );
    for (flag, limit) in [
        ("--max", "2"),
        ("--max-nesting", "1"),
        ("--max-statements", "2"),
        ("--max-cognitive", "2"),
    ] {
        let output = run(&["--json", flag, limit, "--stdin", "a.js"], source);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{flag}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let scope = &result["report"]["scopes"][1];
        assert_eq!(scope["max_nesting"], 2);
        assert_eq!(scope["statement_count"], 3);
        assert_eq!(scope["cognitive_complexity"], 3);
    }
    let output = run(
        &[
            "--max",
            "3",
            "--max-nesting",
            "2",
            "--max-statements",
            "3",
            "--max-cognitive",
            "3",
            "--stdin",
            "a.js",
        ],
        source,
    );
    assert!(output.status.success());
}

#[test]
fn service_returns_all_metrics_after_buffer_edits() {
    let before = "function f() { return 1; }";
    let after = "function f() { if (a) { if (b) { run(); } } }";
    let requests = [
        serde_json::json!({"op":"open", "path":"a.ts", "source":before}),
        serde_json::json!({"op":"edit", "path":"a.ts", "version":0, "start_byte":0, "end_byte":before.len(), "text":after}),
    ];
    let output = run(
        &["serve"],
        &requests
            .iter()
            .map(|r| format!("{r}\n"))
            .collect::<String>(),
    );
    assert!(output.status.success());
    let results: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let scope = &results[1]["report"]["scopes"][1];
    assert_eq!(results[1]["version"], 1);
    assert_eq!(scope["complexity"], 3);
    assert_eq!(scope["max_nesting"], 2);
    assert_eq!(scope["statement_count"], 3);
    assert_eq!(scope["cognitive_complexity"], 3);
}

#[test]
fn cli_selects_metrics_for_text_and_json_output() {
    let source = "function f() { if (a) { while (b) { run(); } } }";
    let output = run(
        &["--metrics", "cyclomatic,cognitive", "--stdin", "a.js"],
        source,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("cyclomatic=3 cognitive=3"), "{text}");
    assert!(!text.contains("nesting="));
    assert!(!text.contains("statements="));
    let output = run(
        &[
            "--json",
            "--metrics",
            "nesting",
            "--max-nesting",
            "1",
            "--stdin",
            "a.js",
        ],
        source,
    );
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let scope = &json["report"]["scopes"][1];
    assert_eq!(scope["max_nesting"], 2);
    for key in ["complexity", "statement_count", "cognitive_complexity"] {
        assert!(scope.get(key).is_none(), "{key}");
    }
}

#[test]
fn cli_rejects_limits_for_disabled_metrics_and_invalid_selections() {
    for (metric, flag) in [
        ("nesting", "--max"),
        ("cognitive", "--max-nesting"),
        ("cyclomatic", "--max-statements"),
        ("statements", "--max-cognitive"),
    ] {
        for limit in ["1".to_owned(), usize::MAX.to_string()] {
            let output = run(&["--metrics", metric, flag, &limit, "--stdin", "a.js"], "");
            assert_eq!(output.status.code(), Some(2), "{metric} {flag} {limit}");
            assert!(
                String::from_utf8(output.stderr)
                    .unwrap()
                    .contains("disabled")
            );
            assert!(output.stdout.is_empty());
        }
    }
    for selection in ["", "cognitve", "nesting,", "all"] {
        let output = run(&["--metrics", selection, "--stdin", "a.js"], "");
        assert_eq!(output.status.code(), Some(2), "{selection}");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("Unknown metric")
        );
    }
}

#[test]
fn service_keeps_each_documents_metric_selection_across_edits() {
    let source = "const f = () => 1;";
    let requests = [
        serde_json::json!({"op":"open","path":"a.ts","source":source,"metrics":["cognitive"]}),
        serde_json::json!({"op":"open","path":"b.ts","source":source}),
        serde_json::json!({"op":"edit","path":"a.ts","version":0,"start_byte":16,"end_byte":17,"text":"x ? 1 : 0"}),
        serde_json::json!({"op":"open","path":"c.ts","source":source,"metrics":["typo"]}),
        serde_json::json!({"op":"open","path":"c.ts","source":source,"metrics":[]}),
    ];
    let output = run(
        &["serve"],
        &requests
            .iter()
            .map(|r| format!("{r}\n"))
            .collect::<String>(),
    );
    assert!(output.status.success());
    let results: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let first = &results[0]["report"]["scopes"][1];
    assert_eq!(first["cognitive_complexity"], 0, "{}", results[0]);
    for key in ["complexity", "max_nesting", "statement_count"] {
        assert!(first.get(key).is_none());
    }
    let default = &results[1]["report"]["scopes"][1];
    for key in [
        "complexity",
        "max_nesting",
        "statement_count",
        "cognitive_complexity",
    ] {
        assert!(default.get(key).is_some());
    }
    let edited = &results[2]["report"]["scopes"][1];
    assert_eq!(edited["cognitive_complexity"], 1);
    assert!(edited.get("complexity").is_none());
    assert_eq!(results[2]["version"], 1);
    assert!(results[3]["error"].is_string());
    let empty = &results[4]["report"]["scopes"][1];
    assert_eq!(empty["name"], "f");
    for key in [
        "complexity",
        "max_nesting",
        "statement_count",
        "cognitive_complexity",
    ] {
        assert!(empty.get(key).is_none());
    }
}

#[test]
fn cli_reports_its_version_without_requiring_input() {
    for flag in ["--version", "-V"] {
        let output = run(&[flag], "");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("ccrs {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
    }
}
