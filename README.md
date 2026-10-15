# ccrs

Cyclomatic complexity, maximum nesting, statement count, and cognitive complexity
for JavaScript, JSX, TypeScript, and TSX. Includes a Rust library, CLI, and persistent
JSON service for editors and coding agents. Accuracy comes first; latency comes second.

## Run

Requires a current stable Rust toolchain and a C compiler for Tree-sitter.
Node.js is only needed for the optional ESLint/SonarJS comparison tests.

```sh
cargo build --release
./target/release/ccrs src/example.ts
./target/release/ccrs --json --max 10 --max-nesting 3 --max-cognitive 15 src/example.ts src/view.tsx
printf 'const f = (x) => x ? 1 : 0;' | ./target/release/ccrs --stdin example.js
./target/release/ccrs serve
```

The CLI accepts explicit file paths. Extensions select the grammar: `.js`, `.jsx`,
`.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`, and `.tsx`. Use `--stdin FILENAME` for an
unsaved buffer; the filename selects the grammar and need not exist.

Text output labels each metric:

```text
example.js:1  cyclomatic=4 nesting=3 statements=3 cognitive=6  f
```

`--json` emits one JSON object per file, each containing `path` and `report`.
All four metrics are enabled by default in the CLI, library, and JSON service.

| CLI limit | JSON / Rust field | Baseline |
| --- | --- | ---: |
| `--max N` | `complexity` (cyclomatic; existing name preserved) | 1 |
| `--max-nesting N` | `max_nesting` | 0 |
| `--max-statements N` | `statement_count` | 0 |
| `--max-cognitive N` | `cognitive_complexity` | 0 |

Exit codes: **0** success, **1** a scope strictly exceeds any supplied limit,
**2** an input error or parser syntax error. Limits apply to all reported scopes,
including the module. There are no default limits. The module measures top-level
code; it is not a sum of the function metrics.

## Select metrics

Use `--metrics` with a comma-separated list of `cyclomatic`, `nesting`,
`statements`, and `cognitive`:

```sh
./target/release/ccrs --metrics cyclomatic,cognitive src/example.ts
./target/release/ccrs --json --metrics nesting --max-nesting 3 src/example.ts
```

Only selected metrics are calculated and displayed. Disabled fields are omitted
from JSON, not reported as zero or null. Parsing, scope discovery, and parser-error
reporting still run. Duplicate names have no additional effect; output keeps the
standard metric order. Empty or unknown CLI selections are rejected. Omit the flag
to calculate all four metrics.

A limit for a disabled metric is an error: for example, `--metrics cognitive --max 5`
would not enforce cyclomatic complexity, so the CLI rejects it. Select the metric
or remove its limit.

## Counting rules

### Cyclomatic complexity

The metric follows [ESLint's classic complexity convention](https://eslint.org/docs/latest/rules/complexity),
with an additional module scope for top-level code. Each scope starts at **1**.
Each of these adds **1**:

- `if` (including `else if`), `for`, `for…in`, `for…of`, `while`, and `do…while`.
- `catch`, each non-default `case`, and each conditional expression (`?:`).
- Each `&&`, `||`, `??`, `&&=`, `||=`, and `??=`.
- Each optional access or call (`?.`), and each parameter/destructuring default.

`else`, `default` switch labels, `try`, `finally`, `return`, `throw`, comments,
string contents, and TypeScript types add nothing. Dead-code branches still count.
Functions, arrows, generators, methods, class field initializers, and class static
blocks have independent scores. Nested scopes do not inflate their parents.
Computed method and field names are evaluated in the surrounding scope.

### Maximum nesting

Follows [ESLint max-depth](https://eslint.org/docs/latest/rules/max-depth): `if`,
loops, `switch`, `try`, and `with` add a level while traversing their contents.
`else if` stays at the same level; `catch` and `finally` share their `try` level.
Plain braces, function bodies, and ternary expressions do not add levels.

### Statement count

Follows [ESLint max-statements](https://eslint.org/docs/latest/rules/max-statements):
sum the number of direct entries in each statement block belonging to the scope.
Comments are excluded. A variable declaration with multiple bindings counts once;
a nested function declaration counts once in its parent, and its body belongs to
its own scope. An expression-bodied arrow has zero statements.

This is a **block-entry count**, not a count of every executable AST statement.
For example, `if (x) run();` counts 1, while `if (x) { run(); }` counts 2. Direct
`case` contents and loop-header declarations do not add entries. A standalone
nested block itself counts once, plus its entries; empty statements count too.
TypeScript type declarations appearing as block entries also count. This convention
is formatting-sensitive to added braces, matching ESLint's behavior.

### Cognitive complexity

Targets the per-function rules in `eslint-plugin-sonarjs` **4.2.2**, rather than
assuming that every language's cognitive-complexity implementation is identical.
See [SonarJS S3776](https://sonarsource.github.io/rspec/#/rspec/S3776/javascript).

- Starts at 0. `if`, loops, `switch`, `catch`, and ternaries add 1 plus their current
  cognitive nesting depth. A switch counts once regardless of its number of cases.
- Nesting increases inside branches, loop bodies, catch bodies, and switch cases.
  Conditions and loop headers do not inherit a level from their own construct.
- `else if` and plain `else` each add 1 without a nesting penalty. Their branch
  bodies still introduce nesting. Labeled `break` and `continue` each add 1.
- Each run of `&&` operators in a connected logical expression adds 1. `||` and
  `??` add nothing but separate runs: `a && b || c && d` adds 2. Parentheses alone
  do not break a run. TypeScript assertions preserve their binding semantics.
- Homogeneous logical chains directly inside JSX expression containers are
  excluded unless an operand is a conditional expression, matching SonarJS.
- Defaults, optional chaining, logical assignments, ordinary calls (including
  recursion), unlabelled jumps, `return`, `throw`, `try`, and `finally` add nothing.

For all four metrics, nested functions, field initializers, and static blocks
remain independent scopes. Computed names belong to the surrounding scope. This
scope policy intentionally extends the reference rules: ESLint max-statements
does not report module/static-block counts, and SonarJS can attribute field/static
code to an enclosing function and computes its file aggregate differently. We do
not claim compatibility with SonarJS file aggregates.

Three flat `if` statements score cyclomatic **4**, nesting **1**, statements **3**,
and cognitive **3**. Nesting those same three conditions scores **4**, **3**, **3**,
and **6**, respectively.

### Report structure and incomplete code

Reports contain `has_errors` and `scopes`. Each scope has the selected metric fields
listed above, plus `name`, `line`, `start_byte`, and `end_byte`. Byte ranges are half-open UTF-8 ranges;
lines are one-based. Anonymous functions get a variable name where directly
assigned, otherwise `<anonymous>`. Initializers and static blocks use descriptive
placeholder names; byte ranges distinguish scopes with the same name.

Tree-sitter can recover from incomplete code. When `has_errors` is true, **scores
are provisional**. This is not a JavaScript validator or TypeScript type checker:
a false flag only means the parser did not report an error. Supported syntax is
bounded by the pinned grammar versions. The comparison suite checks selected
fixtures; it does not establish full reference equivalence for every language feature.

## Rust API

```rust
use ccrs::{Document, Language};

let mut doc = Document::new(Language::TypeScript, "const f = () => 1;".into())?;
assert_eq!(doc.analyze().scopes[1].complexity, Some(1));
doc.edit(16, 17, "x ? 1 : 0")?;
assert_eq!(doc.analyze().scopes[1].complexity, Some(2));
assert_eq!(doc.analyze().scopes[1].max_nesting, Some(0));
assert_eq!(doc.analyze().scopes[1].statement_count, Some(0));
assert_eq!(doc.analyze().scopes[1].cognitive_complexity, Some(1));
# Ok::<(), String>(())
```

Select metrics for an individual analysis without reparsing or changing the document:

```rust
use ccrs::{Document, Language, Metric};

let doc = Document::new(Language::TypeScript, "const f = () => x ? 1 : 0;".into())?;
let report = doc.analyze_with(&[Metric::Cognitive]);
assert_eq!(report.scopes[1].cognitive_complexity, Some(1));
assert_eq!(report.scopes[1].complexity, None);
# Ok::<(), String>(())
```

`analyze()` calculates all four metrics; `analyze_with(&Metric::ALL)` is equivalent.
An empty slice returns scope locations and parser status without metric values.
Metric fields in the Rust `Scope` API are now `Option<usize>` rather than `usize`:
`Some(score)` means calculated, including zero, and `None` means disabled. This is
a Rust source API change; default JSON values remain numbers with the same keys.

`Document` retains the source, parser, and syntax tree. Edits must use UTF-8 byte
offsets on character boundaries; invalid ranges return an error without changing
the document. Tree-sitter updates the parse incrementally. Analysis walks the
whole resulting tree using an iterative traversal, avoiding recursive Rust calls.

## JSON protocol

Run one long-lived `ccrs serve` process. Send one request per line on stdin; each
response is flushed immediately to stdout. Requests are processed sequentially,
and responses correspond in input order. JSON strings must escape embedded newlines.

```jsonl
{"op":"open","path":"example.ts","source":"const f = () => 1;"}
{"op":"edit","path":"example.ts","version":0,"start_byte":16,"end_byte":17,"text":"x ? 1 : 0"}
{"op":"close","path":"example.ts"}
```

`open` returns `{ "path": "…", "version": 0, "report": { … } }`.
It accepts an optional per-document metric selection:

```jsonl
{"op":"open","path":"example.ts","source":"const f = () => 1;","metrics":["cyclomatic","cognitive"]}
```

Omitting `metrics` (or sending `null`) enables all four; `[]` requests only scope
locations and parser status. Unknown names are rejected. The selection persists
across all edits to that document and is independent of other open documents.
Close and reopen the document to change its selection. `metrics` is accepted on
`open` only; subsequent `edit` responses include the selected fields.

An edit supplies the **current** version and returns the next version and report.
Every successful edit advances the version, including an empty edit. Syntax errors
in an edited buffer still produce a successful report with `has_errors: true`.
`close` releases the document and returns `{ "path": "…", "closed": true }`.

Malformed requests, unsupported extensions, duplicate opens, unknown documents,
stale versions, and invalid ranges return `{ "error": "…" }` without terminating
the service. Failed edits preserve the current version and document. To replace
an entire buffer, edit its full byte range. To reopen a path, close it first.
Paths are exact document keys; the service does not read or write those files.

Editor clients must convert UTF-16 positions to UTF-8 byte offsets. Keep one
request in flight per document and associate replies in order. This is a local
stdio protocol, not LSP or MCP. Editor adapters and filesystem watching are not
included in this initial version.

## Verify

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo bench --bench latency
```

The tests exercise public APIs and the real CLI, including scope ownership,
branch counting, nesting, cognitive operator sequences, statement conventions,
TypeScript assertion precedence, all 16 metric selections, protocol version checks, immediate response
flushing, and incremental-versus-fresh parsing at every UTF-8 boundary of a
fixture with Unicode and CRLF line endings.

For an independent reference, install test-only dependencies outside the project:

```sh
npm install --prefix /tmp/ccrs-oracle eslint@10.12.0 @typescript-eslint/parser@8.71.2 eslint-plugin-sonarjs@4.2.2
cargo build
node scripts/check-eslint.cjs /tmp/ccrs-oracle
node scripts/check-metrics.cjs /tmp/ccrs-oracle
```

The cyclomatic suite compares scope score multisets against ESLint on 61 fixtures.
The new metrics suite checks 213 JS/TS/TSX fixtures against ESLint max-depth, ESLint
max-statements, and SonarJS cognitive complexity. It matches individual functions
by diagnostic source location and checks zero scores too. Rust tests separately
verify the extended scope policy, including module and class initializer scopes.

## Latency measurements

Measured locally on macOS ARM64 with Rust 1.98.0, release mode. The benchmark
creates synthetic files with 10, 100, or 1,000 functions, warms up for 20 runs,
and samples 200 edits per selection (all metrics and cyclomatic only). Each edit replaces a logical operator near EOF and includes
source editing, incremental parsing, and analysis of all four metrics.

| Grammar | 800 B: median | 8 KB: median | 82 KB: median | 82 KB: p95 |
| --- | ---: | ---: | ---: | ---: |
| JavaScript | 0.092 ms | 0.879 ms | 8.942 ms | 9.288 ms |
| TypeScript | 0.086 ms | 0.902 ms | 9.246 ms | 9.451 ms |
| TSX | 0.093 ms | 0.931 ms | 9.384 ms | 9.625 ms |

With only cyclomatic complexity selected, median edit-plus-analysis latency on
the same 82 KB fixture was **4.013 ms** for JavaScript, **4.215 ms** for TypeScript,
and **4.265 ms** for TSX, compared with roughly 9 ms for all four metrics. Disabled
metrics skip both their counting logic and their metric-specific nesting tracking.

These are synthetic library measurements, not editor latency guarantees. They
exclude process startup, JSON serialization, transport, and rendering. Cold parse
plus analysis of the 82 KB fixture took roughly 15–16 ms at the median. Timings
vary with machine load; the benchmark emits p50/p95/p99 for both operations.

Parsing is incremental, but source copying, position calculation, and complexity
analysis still have linear work. Changed-scope caching and a more efficient text
buffer are future optimizations if representative workloads justify them.
