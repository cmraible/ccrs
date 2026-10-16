# ccrs

Code complexity metrics for JavaScript and TypeScript, written in Rust.

ccrs measures individual functions and top-level code so you can find complicated
code, set limits in CI, and give editors or coding agents feedback as code changes.
It supports JavaScript, JSX, TypeScript, and TSX, with a CLI, a Rust library, and a
persistent JSON service backed by Tree-sitter's incremental parser.

| Metric | What it measures |
| --- | --- |
| Cyclomatic complexity | Branching decisions, plus a baseline of one per scope |
| Maximum nesting | The deepest level of nested control flow |
| Statement count | Entries in statement blocks, following ESLint's convention |
| Cognitive complexity | Control-flow complexity with additional weight for nesting |

All four metrics are enabled by default. Functions are measured independently;
nested function bodies do not inflate their parents' scores.

## Installation

Requires a current stable Rust toolchain and a C compiler.
Install directly from GitHub:

```sh
cargo install --git https://github.com/cmraible/ccrs.git --locked ccrs
```

Or clone the repository and install locally:

```sh
git clone https://github.com/cmraible/ccrs.git
cd ccrs
cargo install --path . --locked
```

ccrs is not currently published on crates.io. Both methods build the `ccrs`
binary and install it into Cargo's bin directory (usually `~/.cargo/bin`).

## Usage

Analyze one or more files:

```sh
ccrs src/example.ts
ccrs src/api.ts src/components/Button.tsx
```

Read code from stdin, using a filename to select the language:

```sh
printf 'const choose = (value) => value ? "yes" : "no";' | ccrs --stdin example.js
```

Output:

```text
example.js:1  cyclomatic=1 nesting=0 statements=1 cognitive=0  <module>
example.js:1  cyclomatic=2 nesting=0 statements=0 cognitive=1  choose
```

Each line identifies the file, line number, scores, and scope name. `<module>`
measures top-level code; it is not the sum of the file's function scores.

The CLI accepts explicit file paths, not directories. Supported extensions are
`.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`, and `.tsx`.

### Choose metrics

Calculate only the metrics you need:

```sh
ccrs --metrics cyclomatic,cognitive src/example.ts
ccrs --metrics nesting src/example.ts
```

Available names: `cyclomatic`, `nesting`, `statements`, and `cognitive`.
Unselected metrics are not calculated or included in output.

### Set limits in CI

```sh
ccrs --max 10 --max-nesting 3 --max-statements 30 --max-cognitive 15 src/example.ts
```

These are example limits; ccrs does not enforce any by default. A limit applies
to every reported scope and requires its metric to be enabled.

Exit codes:

- **0:** analysis succeeded and all supplied limits passed.
- **1:** at least one scope exceeded a supplied limit.
- **2:** an input or parser syntax error occurred.

### Get JSON output

```sh
ccrs --json --metrics cyclomatic,cognitive src/example.ts src/api.ts
```

Outputs one JSON object per file, containing `path` and `report`. Reports include
scope names, source locations, selected scores, and a `has_errors` flag. Scores
are provisional when the parser reports syntax errors.

### Integrate with an editor or agent

```sh
ccrs serve
```

The persistent service accepts newline-delimited JSON requests to open buffers,
apply edits, and close documents. It keeps parse trees between edits and returns
updated metrics. See the [JSON protocol](docs/protocol.md) for request examples
and the [Rust API](docs/rust-api.md) for direct library integration.

## Documentation

- [Metric definitions](docs/metrics.md): counting rules, scope boundaries, and limitations.
- [JSON protocol](docs/protocol.md): buffer editing, versions, and error handling.
- [Rust API](docs/rust-api.md): analysis, metric selection, and incremental edits.
- [Development and validation](docs/development.md): tests, reference comparisons, and benchmarks.
