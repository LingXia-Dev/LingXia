import { test } from 'node:test';
import assert from 'node:assert/strict';
import { highlight, languages } from './highlight.ts';

/** The coloured runs only, as [kind, text]. */
const kinds = (code, language) => highlight(code, language).filter(run => run.kind).map(run => [run.kind, run.text]);

test('the runs always give back the code, neighbours of one kind merged', () => {
  const code = 'const a = 1; // one\nlet b = "two";';
  for (const language of [...languages, 'klingon', undefined]) {
    const runs = highlight(code, language);
    assert.equal(runs.map(run => run.text).join(''), code, String(language));
    runs.slice(1).forEach((run, index) => assert.notEqual(run.kind, runs[index].kind, `${language}: merged`));
  }
});

test('an unknown language, or none, is one plain run', () => {
  assert.deepEqual(highlight('if (x) return 1;', 'klingon'), [{ text: 'if (x) return 1;' }]);
  assert.deepEqual(highlight('if (x) return 1;', undefined), [{ text: 'if (x) return 1;' }]);
  assert.deepEqual(highlight('', 'ts'), []);
});

test('a language by name or file extension, in any case', () => {
  for (const language of ['ts', 'TypeScript', ' tsx ', 'js', 'mjs']) assert.deepEqual(kinds('return', language), [['keyword', 'return']], language);
});

test('script: keywords, strings across a template, numbers, types by capital, comments', () => {
  assert.deepEqual(kinds('export const n: number = 0x1F; // hex\nnew Map(`a\nb`) /* c */', 'ts'), [
    ['keyword', 'export'], ['keyword', 'const'], ['type', 'number'], ['number', '0x1F'], ['comment', '// hex'],
    ['keyword', 'new'], ['type', 'Map'], ['string', '`a\nb`'], ['comment', '/* c */'],
  ]);
});

test('a string\'s escaped quote does not end it; an unclosed one ends at its line', () => {
  assert.deepEqual(kinds('"a\\"b" x', 'json'), [['string', '"a\\"b"']]);
  assert.deepEqual(kinds('x = "open\ny = 1', 'python'), [['string', '"open'], ['number', '1']]);
});

test('json, python, shell, sql, css, markup and rust read their own way', () => {
  assert.deepEqual(kinds('{"ok": true, "n": -1.5e3}', 'json'), [['string', '"ok"'], ['number', 'true'], ['string', '"n"'], ['number', '1.5e3']]);
  assert.deepEqual(kinds('def f():\n    """doc\n    more"""\n    return None', 'python'), [['keyword', 'def'], ['string', '"""doc\n    more"""'], ['keyword', 'return'], ['number', 'None']]);
  assert.deepEqual(kinds('echo a#b # note', 'bash'), [['comment', '# note']], 'a # inside a word is not a comment');
  assert.deepEqual(kinds('Select id FROM t -- x', 'sql'), [['keyword', 'Select'], ['keyword', 'FROM'], ['comment', '-- x']]);
  assert.deepEqual(kinds('@media (min-width: 40px) { a { color: red !important } }', 'css'), [['keyword', '@media'], ['number', '40px'], ['keyword', '!important']]);
  assert.deepEqual(kinds('<a href="/x">if <b>1</b></a><!-- c -->', 'html'), [['type', 'a'], ['string', '"/x"'], ['type', 'b'], ['type', 'b'], ['type', 'a'], ['comment', '<!-- c -->']], 'text between tags is not code');
  assert.deepEqual(kinds("fn f<'a>(s: &'a str) -> char { 'c' }", 'rust'), [['keyword', 'fn'], ['type', 'str'], ['type', 'char'], ['string', "'c'"]], 'a lifetime is not a string');
});

test('never throws, and malformed input still gives back the code', () => {
  const samples = ['"', '/*', '<', '<a', '<!--', '`${', "'", '"""', '0x', '.5', '\\', '@', '#', '!', '-', '\u0000', '\ud800', 'é ü 中文 🙂'];
  for (const language of languages)
    for (const sample of samples) {
      const runs = highlight(sample, language);
      assert.equal(runs.map(run => run.text).join(''), sample, `${language}: ${JSON.stringify(sample)}`);
    }
});

test('time grows with the code, not faster, on pathological input', () => {
  const time = code => {
    const start = performance.now();
    for (const language of ['ts', 'html', 'python', 'css', 'rust', 'sql', 'bash']) highlight(code, language);
    return performance.now() - start;
  };
  const shapes = [n => '"\\'.repeat(n), n => '/*'.repeat(n), n => '<a '.repeat(n), n => 'a'.repeat(n), n => '1.'.repeat(n), n => "'a".repeat(n), n => '#'.repeat(n), n => '\n'.repeat(n)];
  for (const shape of shapes) {
    time(shape(2000));
    const [small, large] = [time(shape(20000)), time(shape(160000))];
    assert.ok(large < small * 8 * 4 + 50, `${JSON.stringify(shape(2))}: ${small.toFixed(1)} ms then ${large.toFixed(1)} ms for 8 times the code`);
  }
});
