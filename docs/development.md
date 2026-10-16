# Development and validation

[Back to the README](../README.md)

Run these commands from the repository root. Node.js is needed only for the
optional reference comparisons. Rust 1.90 is the minimum supported toolchain.

GitHub Actions checks formatting and Clippy, tests and builds on Linux, macOS,
and Windows, and runs the reference comparisons on Linux. A separate Linux
matrix entry tests Rust 1.90 to keep the minimum version verified.

## Checks

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
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
cargo build --locked
node scripts/check-eslint.cjs /tmp/ccrs-oracle
node scripts/check-metrics.cjs /tmp/ccrs-oracle
```

The cyclomatic suite compares scope score multisets against ESLint on 61 fixtures.
The additional metrics suite checks 213 JS/TS/TSX fixtures against ESLint max-depth, ESLint
max-statements, and SonarJS cognitive complexity. It matches individual functions
by diagnostic source location and checks zero scores too. Rust tests separately
verify the extended scope policy, including module and class initializer scopes.

## Latency measurements

Measured locally on macOS ARM64 with Rust 1.98.0, release mode. The benchmark
creates synthetic files with 10, 100, or 1,000 functions, warms up for 20 runs,
and samples 200 edits per selection (all metrics and cyclomatic only). Each edit replaces a logical operator near EOF and includes
source editing, incremental parsing, and analysis of the selected metrics.
The table below reports all four metrics enabled.

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
