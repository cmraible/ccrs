# Metric definitions

[Back to the README](../README.md)

## Cyclomatic complexity

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

## Maximum nesting

Follows [ESLint max-depth](https://eslint.org/docs/latest/rules/max-depth): `if`,
loops, `switch`, `try`, and `with` add a level while traversing their contents.
`else if` stays at the same level; `catch` and `finally` share their `try` level.
Plain braces, function bodies, and ternary expressions do not add levels.

## Statement count

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

## Cognitive complexity

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

## Report structure and incomplete code

Reports contain `has_errors` and `scopes`. Each scope has the selected metric fields
(`complexity`, `max_nesting`, `statement_count`, and `cognitive_complexity`), plus `name`, `line`, `start_byte`, and `end_byte`. Byte ranges are half-open UTF-8 ranges;
lines are one-based. Anonymous functions get a variable name where directly
assigned, otherwise `<anonymous>`. Initializers and static blocks use descriptive
placeholder names; byte ranges distinguish scopes with the same name.

Tree-sitter can recover from incomplete code. When `has_errors` is true, **scores
are provisional**. This is not a JavaScript validator or TypeScript type checker:
a false flag only means the parser did not report an error. Supported syntax is
bounded by the pinned grammar versions. The comparison suite checks selected
fixtures; it does not establish full reference equivalence for every language feature.

## Metric selection

Use `--metrics` with a comma-separated list of `cyclomatic`, `nesting`,
`statements`, and `cognitive`:

```sh
ccrs --metrics cyclomatic,cognitive src/example.ts
ccrs --json --metrics nesting --max-nesting 3 src/example.ts
```

Only selected metrics are calculated and displayed. Disabled fields are omitted
from JSON, not reported as zero or null. Parsing, scope discovery, and parser-error
reporting still run. Duplicate names have no additional effect; output keeps the
standard metric order. Empty or unknown CLI selections are rejected. Omit the flag
to calculate all four metrics.

A limit for a disabled metric is an error: for example, `--metrics cognitive --max 5`
would not enforce cyclomatic complexity, so the CLI rejects it. Select the metric
or remove its limit.
