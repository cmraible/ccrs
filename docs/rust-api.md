# Rust API

[Back to the README](../README.md)

```rust
use ccrs::{Document, Language};

fn main() -> Result<(), String> {
    let mut doc = Document::new(Language::TypeScript, "const f = () => 1;".into())?;
    assert_eq!(doc.analyze().scopes[1].complexity, Some(1));
    doc.edit(16, 17, "x ? 1 : 0")?;
    assert_eq!(doc.analyze().scopes[1].complexity, Some(2));
    assert_eq!(doc.analyze().scopes[1].max_nesting, Some(0));
    assert_eq!(doc.analyze().scopes[1].statement_count, Some(0));
    assert_eq!(doc.analyze().scopes[1].cognitive_complexity, Some(1));
    Ok(())
}
```

Select metrics for an individual analysis without reparsing or changing the document:

```rust
use ccrs::{Document, Language, Metric};

fn main() -> Result<(), String> {
    let doc = Document::new(Language::TypeScript, "const f = () => x ? 1 : 0;".into())?;
    let report = doc.analyze_with(&[Metric::Cognitive]);
    assert_eq!(report.scopes[1].cognitive_complexity, Some(1));
    assert_eq!(report.scopes[1].complexity, None);
    Ok(())
}
```

`analyze()` calculates all four metrics; `analyze_with(&Metric::ALL)` is equivalent.
An empty slice returns scope locations and parser status without metric values.
Metric fields in the Rust `Scope` API are `Option<usize>`:
`Some(score)` means calculated, including zero, and `None` means disabled. In JSON, calculated values are numbers and disabled fields are omitted.

`Document` retains the source, parser, and syntax tree. Edits must use UTF-8 byte
offsets on character boundaries; invalid ranges return an error without changing
the document. Tree-sitter updates the parse incrementally. Analysis walks the
whole resulting tree using an iterative traversal, avoiding recursive Rust calls.
