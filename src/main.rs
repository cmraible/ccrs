mod service;

use ccrs::{Document, Language};
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
    let mut max = usize::MAX;
    let mut max_nesting = usize::MAX;
    let mut max_statements = usize::MAX;
    let mut max_cognitive = usize::MAX;
    let mut stdin_path = None;
    let mut paths = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
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
                } = limit;
            }
            "--stdin" => stdin_path = Some(args.next().ok_or("--stdin requires a filename")?),
            "--help" | "-h" => {
                println!(
                    "ccrs [--json] [--max N] FILE...\nccrs [--json] [--max N] --stdin FILENAME\nccrs serve\n\nLimits: --max N (cyclomatic), --max-nesting N, --max-statements N, --max-cognitive N\n\nExit codes: 0 success, 1 threshold exceeded, 2 input or syntax error."
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
        let report = Document::new(language, source)?.analyze();
        if report.scopes.iter().any(|s| {
            s.complexity > max
                || s.max_nesting > max_nesting
                || s.statement_count > max_statements
                || s.cognitive_complexity > max_cognitive
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
                println!(
                    "{path}:{}  cyclomatic={} nesting={} statements={} cognitive={}  {}",
                    scope.line,
                    scope.complexity,
                    scope.max_nesting,
                    scope.statement_count,
                    scope.cognitive_complexity,
                    scope.name
                );
            }
            if report.has_errors {
                eprintln!("{path}: syntax errors; scores are provisional");
            }
        }
    }
    Ok(exit)
}
