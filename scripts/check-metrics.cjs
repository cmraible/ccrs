// Compare each function by source location, including zero-score functions.
const { createRequire } = require('node:module');
const { resolve } = require('node:path');
const { execFileSync } = require('node:child_process');
const assert = require('node:assert/strict');
const oracle = createRequire(resolve(process.argv[2] || '/tmp/ccrs-oracle', 'package.json'));
const { Linter } = oracle('eslint');
const tsParser = oracle('@typescript-eslint/parser');
const sonar = oracle('eslint-plugin-sonarjs');
const linter = new Linter();
const binary = resolve('target/debug/ccrs');
const bodies = [
  '', 'return 0;', 'const a = 1, b = 2; return a + b;',
  '// comment\n/* comment */', '{ run(); } ;',
  'if (a) run();', 'if (a) { run(); }',
  'if (a) {} if (b) {} if (c) {}',
  'if (a) { if (b) { if (c) {} } }',
  'if (a) {} else if (b) {} else {}',
  'while (a) { if (b) {} else if (c) {} else {} }',
  'if (a) {} else if (b) { if (c) {} }',
  'try { if (a) {} } catch (e) { while (b) {} } finally { if (c) {} }',
  'switch (x) { case 1: if (a) {} break; case 2: break; default: break; }',
  'switch (x) { case 1: { if (a) {} break; } }',
  'outer: for (;;) { for (const x of xs) { if (a) continue outer; } }',
  'for (const k in obj) { do { if (a) {} } while (b); }',
  'return a ? (b ? c : d) : e;', 'return (a ? b : c) ? d : e;',
  'return a && b && c;', 'return a || b || c;', 'return a ?? b;',
  'return a && b || c && d;', 'return a && (b || c) && d;',
  'return a && (b && c);', 'return a && call(b && c);',
  'return a?.b?.() ?? c;', 'a &&= b; a ||= b; a ??= b;', 'return f(a);',
  'if (a) { const inner = () => { if (b) { if (c) {} } }; }',
  'function inner() { run(); stop(); } const arrow = () => 0;',
  'const obj = { method() { while (a) { if (b) {} } } };',
];
for (const left of ['&&', '||', '??']) {
  for (const middle of ['&&', '||', '??']) {
    for (const right of ['&&', '||', '??']) {
      bodies.push(`return (a ${left} b) ${middle} (c ${right} d);`);
    }
  }
}
const jsx = [
  'a && <Item />', 'a && b && <Item />', '(a && b)',
  'a && b || c && <Item />', 'a && (b ? <A /> : <B />)', 'a && call(b && c)',
];
const metrics = {
  'max-depth': ['max_nesting', /deeply \((\d+)\)/],
  'max-statements': ['statement_count', /statements \((\d+)\)/],
  'sonarjs/cognitive-complexity': ['cognitive_complexity', /from (\d+) to/],
};
let checked = 0;
for (const extension of ['js', 'ts', 'tsx']) {
  const fixtures = bodies.map(body => `function f(x = 0) {\n${body}\n}`);
  if (extension !== 'ts') fixtures.push(...jsx.map(expr => `const View = () => <div>{${expr}}</div>;`));
  if (extension !== 'js') fixtures.push(
    'function f(x?: number): number { return x ?? 0; }',
    'function f() { type T<X> = X extends string ? true : false; interface I {} return 0; }',
    'const f = (x: number = 0) => x ? 1 : 2;',
    ...[
      '(a && b as boolean) && c', '((a && b) as boolean) && c',
      '(a && b satisfies boolean) && c', '((a && b) satisfies boolean) && c',
      '(a && b as unknown as boolean) && c', '((a && b) as unknown as boolean) && c',
      '(a && b!) && c', '((a && b)!) && c',
    ].map(expression => `function f() { return ${expression}; }`),
  );
  if (extension === 'ts') fixtures.push(
    'function f() { return <boolean>(a && b) && c; }',
    'function f() { return <boolean>a && b && c; }',
  );
  for (const source of fixtures) {
    const { report } = JSON.parse(execFileSync(binary, ['--json', '--stdin', `fixture.${extension}`], { input: source, encoding: 'utf8' }));
    assert.equal(report.has_errors, false, source);
    const messages = linter.verify(source, [{
      files: ['**/*.*'],
      languageOptions: { ecmaVersion: 'latest', sourceType: 'module', parserOptions: { ecmaFeatures: { jsx: true } }, ...(extension === 'js' ? {} : { parser: tsParser }) },
      plugins: { sonarjs: sonar },
      rules: { 'max-depth': ['error', 0], 'max-statements': ['error', 0], 'sonarjs/cognitive-complexity': ['error', 0] },
    }], { filename: `fixture.${extension}` });
    const scopes = report.scopes.slice(1);
    const expected = scopes.map(() => ({ max_nesting: 0, statement_count: 0, cognitive_complexity: 0 }));
    for (const message of messages) {
      assert(!message.fatal, JSON.stringify(message));
      const [metric, pattern] = metrics[message.ruleId] || [];
      assert(metric, JSON.stringify(message));
      const lines = source.split('\n');
      const prefix = lines.slice(0, message.line - 1).map(line => `${line}\n`).join('') + lines[message.line - 1].slice(0, message.column - 1);
      const offset = Buffer.byteLength(prefix);
      // The innermost scope containing the diagnostic owns the score.
      const owner = scopes.map((scope, index) => ({ scope, index }))
        .filter(({ scope }) => scope.start_byte <= offset && offset < scope.end_byte)
        .sort((a,b) => (a.scope.end_byte - a.scope.start_byte) - (b.scope.end_byte - b.scope.start_byte))[0];
      assert(owner, JSON.stringify(message));
      const score = Number(message.message.match(pattern)[1]);
      expected[owner.index][metric] = Math.max(expected[owner.index][metric], score);
    }
    for (let i = 0; i < scopes.length; i++) {
      for (const metric of Object.keys(expected[i])) {
        assert.equal(scopes[i][metric], expected[i][metric], `${extension}: ${metric} for ${scopes[i].name}: ${source}`);
      }
    }
    checked++;
  }
}
console.log(`Matched ESLint nesting/statements and SonarJS cognitive scores by function location for ${checked} fixtures.`);
