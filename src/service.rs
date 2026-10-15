use ccrs::{Document, Metric};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{self, BufRead, Write},
};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Request {
    Open {
        path: String,
        source: String,
        metrics: Option<Vec<Metric>>,
    },
    Edit {
        path: String,
        version: u64,
        start_byte: usize,
        end_byte: usize,
        text: String,
    },
    Close {
        path: String,
    },
}

pub fn run() -> Result<(), String> {
    let mut documents = HashMap::new();
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let response = serde_json::from_str(&line)
            .map_err(|e| e.to_string())
            .and_then(|request| handle(&mut documents, request))
            .unwrap_or_else(|error| json!({"error": error}));
        writeln!(stdout, "{response}")
            .and_then(|_| stdout.flush())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn handle(
    documents: &mut HashMap<String, (Document, u64, Vec<Metric>)>,
    request: Request,
) -> Result<Value, String> {
    match request {
        Request::Open {
            path,
            source,
            metrics,
        } => {
            let metrics = metrics.unwrap_or_else(|| Metric::ALL.to_vec());
            if documents.contains_key(&path) {
                return Err("Document is already open; close it before reopening".into());
            }
            let document = Document::new(super::language(&path)?, source)?;
            let result =
                json!({"path": path, "version": 0, "report": document.analyze_with(&metrics)});
            documents.insert(path, (document, 0, metrics));
            Ok(result)
        }
        Request::Edit {
            path,
            version,
            start_byte,
            end_byte,
            text,
        } => {
            let (document, current, metrics) =
                documents.get_mut(&path).ok_or("Document is not open")?;
            if version != *current {
                return Err(format!(
                    "Stale version: expected {current}, received {version}"
                ));
            }
            let next = current.checked_add(1).ok_or("Document version overflow")?;
            document.edit(start_byte, end_byte, &text)?;
            *current = next;
            Ok(json!({"path": path, "version": current, "report": document.analyze_with(metrics)}))
        }
        Request::Close { path } => {
            documents.remove(&path).ok_or("Document is not open")?;
            Ok(json!({"path": path, "closed": true}))
        }
    }
}
