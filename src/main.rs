mod service;

use ccrs::{Document, Language, Metric};
use std::{
    io::{self, Read},
    path::Path,
    process::ExitCode,
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("ccrs: {error}");
            ExitCode::from(2)
        }
    }
}

fn language(path: &str) -> Result<Language, String> {
    match Path::new(path).extension().and_then(|s| s.to_str()) {
        Some("js" | "jsx" | "mjs" | "cjs") => Ok(Language::JavaScript),
        Some("ts" | "mts" | "cts") => Ok(Language::TypeScript),
        Some("tsx") => Ok(Language::Tsx),
        _ => Err(format!("Unsupported file extension: {path}")),
    }
}

fn run() -> Result<u8, String> {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().is_some_and(|arg| arg == "serve") {
        args.next();
        if args.next().is_some() {
            return Err("serve takes no arguments".into());
        }
        service::run()?;
        return Ok(0);
    }
    let mut json = false;
    let mut metrics = Metric::ALL.to_vec();
    let mut max = None;
    let mut max_nesting = None;
    let mut max_statements = None;
    let mut max_cognitive = None;
    let mut stdin_path = None;
    let mut paths = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--metrics" => {
                metrics = args
                    .next()
                    .ok_or("--metrics requires a comma-separated list")?
                    .split(',')
                    .map(|name| name.trim().parse())
                    .collect::<Result<Vec<Metric>, _>>()?;
            }
            "--max" | "--max-nesting" | "--max-statements" | "--max-cognitive" => {
                let limit = args
                    .next()
                    .ok_or_else(|| format!("{arg} requires an integer"))?
                    .parse()
                    .map_err(|_| format!("Invalid {arg}"))?;
                *match arg.as_str() {
                    "--max-nesting" => &mut max_nesting,
                    "--max-statements" => &mut max_statements,
                    "--max-cognitive" => &mut max_cognitive,
                    _ => &mut max,
                } = Some(limit);
            }
            "--stdin" => stdin_path = Some(args.next().ok_or("--stdin requires a filename")?),
            "--help" | "-h" => {
                println!(
                    "ccrs [--json] [--max N] FILE...\nccrs [--json] [--max N] --stdin FILENAME\nccrs serve\n\nSelection: --metrics cyclomatic,nesting,statements,cognitive (all by default)\n\nLimits: --max N (cyclomatic), --max-nesting N, --max-statements N, --max-cognitive N\n\nExit codes: 0 success, 1 threshold exceeded, 2 input or syntax error."
                );
                return Ok(0);
            }
            "--" => {
                paths.extend(args);
                break;
            }
            _ if arg.starts_with('-') => return Err(format!("Unknown option: {arg}")),
            _ => paths.push(arg),
        }
    }
    for (metric, flag, limit) in [
        (Metric::Cyclomatic, "--max", max),
        (Metric::Nesting, "--max-nesting", max_nesting),
        (Metric::Statements, "--max-statements", max_statements),
        (Metric::Cognitive, "--max-cognitive", max_cognitive),
    ] {
        if limit.is_some() && !metrics.contains(&metric) {
            return Err(format!(
                "{flag} applies to disabled metric '{}'; include it in --metrics",
                metric.name()
            ));
        }
    }
    if stdin_path.is_some() && !paths.is_empty() {
        return Err("Use files or --stdin, not both".into());
    }
    let from_stdin = stdin_path.is_some();
    if let Some(path) = stdin_path {
        paths.push(path);
    }
    if paths.is_empty() {
        return Err("Provide a file or --stdin FILENAME; see --help".into());
    }
    let mut exit = 0;
    for path in paths {
        let language = language(&path)?;
        let source = if from_stdin {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|e| e.to_string())?;
            source
        } else {
            std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?
        };
        let report = Document::new(language, source)?.analyze_with(&metrics);
        if report.scopes.iter().any(|s| {
            s.complexity
                .zip(max)
                .is_some_and(|(value, limit)| value > limit)
                || s.max_nesting
                    .zip(max_nesting)
                    .is_some_and(|(value, limit)| value > limit)
                || s.statement_count
                    .zip(max_statements)
                    .is_some_and(|(value, limit)| value > limit)
                || s.cognitive_complexity
                    .zip(max_cognitive)
                    .is_some_and(|(value, limit)| value > limit)
        }) {
            exit = exit.max(1);
        }
        if report.has_errors {
            exit = 2;
        }
        if json {
            println!("{}", serde_json::json!({"path": path, "report": report}));
        } else {
            for scope in &report.scopes {
                let values = [
                    scope.complexity,
                    scope.max_nesting,
                    scope.statement_count,
                    scope.cognitive_complexity,
                ];
                let scores = Metric::ALL
                    .into_iter()
                    .zip(values)
                    .filter_map(|(metric, value)| value.map(|v| format!("{}={v}", metric.name())))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("{path}:{}  {scores}  {}", scope.line, scope.name);
            }
            if report.has_errors {
                eprintln!("{path}: syntax errors; scores are provisional");
            }
        }
    }
    Ok(exit)
}
