use ccrs::{Document, Language};
use std::{hint::black_box, time::Instant};

fn measure(mut work: impl FnMut(), iterations: usize) -> (f64, f64, f64) {
    for _ in 0..20 {
        work();
    }
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        work();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    (
        samples[iterations / 2],
        samples[iterations * 95 / 100],
        samples[iterations * 99 / 100],
    )
}

fn main() {
    println!("language,bytes,functions,operation,p50_ms,p95_ms,p99_ms");
    for language in [Language::JavaScript, Language::TypeScript, Language::Tsx] {
        for count in [10, 100, 1000] {
            let source: String = (0..count).map(|i| format!(
                "export function f{i}(x) {{ if (x && x.ok) return x.value ?? 0; return x ? 1 : 2; }}\n"
            )).collect();
            let (p50, p95, p99) = measure(
                || {
                    black_box(Document::new(language, source.clone()).unwrap().analyze());
                },
                100,
            );
            println!(
                "{language:?},{},{count},cold,{p50:.3},{p95:.3},{p99:.3}",
                source.len()
            );
            let offset = source.rfind("&&").unwrap();
            let mut document = Document::new(language, source.clone()).unwrap();
            let mut alternate = false;
            let (p50, p95, p99) = measure(
                || {
                    alternate = !alternate;
                    document
                        .edit(offset, offset + 2, if alternate { "||" } else { "&&" })
                        .unwrap();
                    black_box(document.analyze());
                },
                200,
            );
            println!(
                "{language:?},{},{count},edit,{p50:.3},{p95:.3},{p99:.3}",
                source.len()
            );
        }
    }
}
