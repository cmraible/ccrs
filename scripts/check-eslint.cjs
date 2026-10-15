// Optional independent oracle. See README for setup; Node is not a runtime dependency.
const { createRequire } = require('node:module');
const { resolve } = require('node:path');
const { execFileSync } = require('node:child_process');
const assert = require('node:assert/strict');
const oracle = createRequire(resolve(process.argv[2] || '/tmp/ccrs-oracle', 'package.json'));
const { Linter } = oracle('eslint');
const tsParser = oracle('@typescript-eslint/parser');
const linter = new Linter();
const binary = resolve('target/debug/ccrs');
const bodies = [
  'return 1;',
  'if (x) {} else if (y) {} else {}',
  'for (;;) { break; } while (x) {} do {} while (y);',
  'for (const v of xs) {} for (const k in x) {}',
  'try {} catch (e) {} finally {}',
  'switch (x) { case 1: case 2: break; default: break; }',
  'return x ? y : z;',
  'return x && y || (z ?? 0);',
  'x &&= y; x ||= y; x ??= y;',
  'return x?.y?.[z]?.();',
  'const {a = 1, b: c = 2} = x; const [d = 3] = y;',
  'return /if|while|a?/.test("&& ??");',
  'const f = (a = 0) => a ? 1 : 0; return x && f(x);',
  'function* g() { if (x) yield x; } return () => y || z;',
  'class C { [x || y] = a && b; field = x ? y : z; static { if (x) y(); } [x ?? y]() { if (z) x(); } }',
  'class C { field = (x = 0) => x ? 1 : 2; get value() { return x ?? 0; } set value(x) { if (x) y(); } }',
  'const obj = { [x || y]() { return a && b; }, f: () => x?.y };',
  'const {a = x ? y : z} = obj; return function named(b = x && y) { return b ?? z; };',
];
let checked = 0;
for (const extension of ['js', 'ts', 'tsx']) {
  const fixtures = bodies.map(body => `function f(x = 0) { ${body} }`);
  if (extension !== 'js') fixtures.push(
    'type T<X> = X extends string ? true : false; function f(x?: number): number { return x ?? 0; }',
    'function f({a = 0}: {a?: number} = {}) { return a ? 1 : 0; }',
    'class C { field: number = x || y; method(x: number = 0): number { return x && y; } }',
  );
  if (extension === 'tsx') fixtures.push('const View = (x: number = 0) => <div>if ? {x ? "a" : "b"}</div>;');
  for (const source of fixtures) {
    const messages = linter.verify(source, [{
      files: ['**/*.*'],
      languageOptions: { ecmaVersion: 'latest', sourceType: 'module', ...(extension === 'js' ? {} : { parser: tsParser }) },
      rules: { complexity: ['error', { max: 0, variant: 'classic' }] },
    }], { filename: `fixture.${extension}` });
    assert(!messages.some(m => m.fatal), JSON.stringify(messages));
    const expected = messages.map(m => {
      assert.equal(m.ruleId, 'complexity', m.message);
      return Number(m.message.match(/complexity of (\d+)/)[1]);
    }).sort((a,b) => a-b);
    const { report } = JSON.parse(execFileSync(binary, ['--json', '--stdin', `fixture.${extension}`], { input: source, encoding: 'utf8' }));
    assert.equal(report.has_errors, false, source);
    const actual = report.scopes.slice(1).map(s => s.complexity).sort((a,b) => a-b);
    assert.deepEqual(actual, expected, `${extension}: ${source}`);
    checked++;
  }
}
console.log(`Matched ESLint classic complexity for ${checked} JS/TS/TSX fixtures.`);
