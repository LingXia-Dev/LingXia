// The `jinshu` command on the corpus files: in process for speed, and spawned through its bin as npx runs it.
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { main } from '../bin/cli.mjs';

const bin = fileURLToPath(new URL('../bin/jinshu.mjs', import.meta.url));
const corpus = name => fileURLToPath(new URL(`./testing/corpus/${name}`, import.meta.url));

function spawned(args, input) {
  const res = spawnSync(process.execPath, [bin, ...args], { encoding: 'utf8', input });
  return { code: res.status, out: res.stdout, err: res.stderr };
}

function jinshu(args, input) {
  let out = '';
  let err = '';
  const io = {
    out: line => (out += `${line}\n`),
    err: line => (err += `${line}\n`),
    write: text => (out += text),
    stdin: () => input ?? '',
  };
  return main(args, io).then(code => ({ code, out, err }));
}

let dir;
before(() => (dir = mkdtempSync(join(tmpdir(), 'jinshu-cli-'))));
after(() => rmSync(dir, { recursive: true, force: true }));
const tmp = (name, text) => {
  const path = join(dir, name);
  if (text !== undefined) writeFileSync(path, text);
  return path;
};

describe('jinshu check', () => {
  it('passes a valid document in each format', async () => {
    for (const name of ['valid/blocks.json', 'valid/blocks.md', 'valid/blocks.xml']) {
      const res = await jinshu(['check', corpus(name)]);
      assert.equal(res.code, 0, res.err);
      assert.match(res.out, /: ok \(0 errors, 0 warnings\)/);
    }
  });

  it('warns that an in-page link in Markdown points at a derived id', async () => {
    const res = await jinshu(['check', corpus('valid/prose.md')]);
    assert.equal(res.code, 0);
    assert.match(res.err, /warning NOT_FOUND #p-\w+: In-page link "#h-intro" points to no block/);
  });

  it('fails an invalid one, naming the code and the id', async () => {
    const res = await jinshu(['check', corpus('invalid/duplicate-id.json')]);
    assert.equal(res.code, 1);
    assert.match(res.err, /error INVALID #p1: Duplicate id "p1"/);
    assert.match(res.out, /invalid \(1 error, 0 warnings\)/);
  });

  it('--json reports the problems as data', async () => {
    const res = await jinshu(['check', corpus('invalid/heading-level.json'), '--json']);
    assert.equal(res.code, 1);
    const report = JSON.parse(res.out);
    assert.equal(report.ok, false);
    assert.deepEqual(report.problems.map(p => [p.code, p.id]), [['INVALID', 'h']]);
  });

  it('--profile markdown holds JSON to what Markdown can say', async () => {
    assert.equal((await jinshu(['check', corpus('valid/rich.json')])).code, 0);
    const res = await jinshu(['check', corpus('valid/rich.json'), '--profile', 'markdown']);
    assert.equal(res.code, 1);
    assert.match(res.err, /error FORMAT #d1/);
  });

  it('reports Markdown reading warnings without failing', async () => {
    const res = await jinshu(['check', corpus('markdown/front-matter.md')]);
    assert.equal(res.code, 0);
    assert.match(res.err, /warning FORMAT: Line 8: Raw HTML/);
  });

  it('refuses a file that is not JSON', async () => {
    const res = await jinshu(['check', tmp('broken.json', '{ "version": 1, ')]);
    assert.equal(res.code, 1);
    assert.match(res.err, /error PARSE: Not JSON/);
  });
});

describe('jinshu outline', () => {
  it('lists headings by level with their ids', async () => {
    const res = await jinshu(['outline', corpus('valid/prose.json')]);
    assert.equal(res.code, 0);
    assert.equal(res.out, 'Week 3: Limits\n  - What a limit is  #h-intro\n    - Back to the top  #h-back\n');
  });

  it('--json', async () => {
    const res = await jinshu(['outline', corpus('valid/cjk.md'), '--json']);
    assert.deepEqual(JSON.parse(res.out).headings.map(h => [h.level, h.text]), [[1, '第一章']]);
  });
});

describe('jinshu diff', () => {
  const prose = JSON.parse(readFileSync(corpus('valid/prose.json'), 'utf8'));

  it('exits 0 for the same document, in two formats', async () => {
    // The second file reads with the first as its previous version, so Markdown keeps the JSON's ids.
    const res = await jinshu(['diff', corpus('valid/lists.json'), corpus('valid/lists.md')]);
    assert.equal(res.code, 0, res.out);
    const same = await jinshu(['diff', corpus('valid/prose.json'), corpus('valid/prose.json')]);
    assert.equal(same.code, 0);
    assert.match(same.out, /0 added, 0 removed, 0 modified, 0 moved, 5 unchanged/);
  });

  it('summarises added, removed and modified blocks, words included', async () => {
    const b = structuredClone(prose);
    b.blocks[1].runs[0].text = 'A limit tells where ';
    b.blocks.splice(2, 1);
    b.blocks.push({ id: 'p-new', type: 'paragraph', runs: [{ text: 'Added.' }] });
    const res = await jinshu(['diff', corpus('valid/prose.json'), tmp('b.json', JSON.stringify(b))]);
    assert.equal(res.code, 1);
    assert.match(res.out, /^~ paragraph #p-def: A limit \[-describes-\]\{\+tells\+\} where/m);
    assert.match(res.out, /^- paragraph #p-marks from 2: See the notes/m);
    assert.match(res.out, /^\+ paragraph #p-new at 4: Added\./m);
    assert.match(res.out, /1 added, 1 removed, 1 modified, 0 moved, 3 unchanged/);
    const json = JSON.parse((await jinshu(['diff', corpus('valid/prose.json'), tmp('b.json'), '--json'])).out);
    assert.deepEqual(json.stats, { added: 1, removed: 1, modified: 1, moved: 0, unchanged: 3 });
  });

  it('keeps Markdown ids between the two files, so an edit is one modified block', async () => {
    const a = readFileSync(corpus('valid/prose.md'), 'utf8');
    const res = await jinshu(['diff', corpus('valid/prose.md'), tmp('b.md', a.replace('Back to the top', 'Back to start'))]);
    assert.equal(res.code, 1);
    assert.match(res.out, /0 added, 0 removed, 1 modified, 0 moved, 4 unchanged/);
  });
});

describe('jinshu apply', () => {
  const ops = JSON.stringify([{ op: 'edit', id: 'p-def', find: 'describes', replace: 'tells' }]);

  it('prints the result in the file’s format', async () => {
    const res = await jinshu(['apply', corpus('valid/prose.json'), tmp('ops.json', ops)]);
    assert.equal(res.code, 0, res.err);
    const doc = JSON.parse(res.out);
    assert.equal(doc.blocks[1].runs[0].text, 'A limit tells where ');
  });

  it('reads ops from stdin, and an object with ops works too', async () => {
    const res = await jinshu(['apply', corpus('valid/prose.xml'), '-'], JSON.stringify({ ops: JSON.parse(ops) }));
    assert.equal(res.code, 0, res.err);
    assert.match(res.out, /<p id="p-def">A limit tells where/);
  });

  it('--write keeps the bytes of untouched Markdown blocks', async () => {
    const original = readFileSync(corpus('markdown/variants.md'), 'utf8');
    const file = tmp('variants.md', original);
    const edit = [{ op: 'edit', id: 'p-18jijxz', find: 'emphasis', replace: 'stress' }];
    const res = await jinshu(['apply', file, tmp('edit.json', JSON.stringify(edit)), '--write']);
    assert.equal(res.code, 0, res.err);
    assert.equal(res.out, '');
    // The edited paragraph is written canonically; every other line keeps its bytes.
    const lines = readFileSync(file, 'utf8').split('\n');
    const was = original.split('\n');
    const edited = was.findIndex(l => l.includes('*emphasis*'));
    assert.equal(lines[edited], 'Some *stress*, **strong** and `code`, see https://example.com or https://example.org.');
    assert.deepEqual(lines.filter((_, i) => i !== edited), was.filter((_, i) => i !== edited));
  });

  it('-o converts to the output file’s format', async () => {
    const out = tmp('out.md');
    const res = await jinshu(['apply', corpus('valid/blocks.json'), tmp('none.json', '[]'), '-o', out]);
    assert.equal(res.code, 0, res.err);
    assert.equal(readFileSync(out, 'utf8'), readFileSync(corpus('valid/blocks.md'), 'utf8'));
  });

  it('refuses the whole batch on any failing op and writes nothing', async () => {
    const file = tmp('prose.json', readFileSync(corpus('valid/prose.json'), 'utf8'));
    const bad = [{ op: 'edit', id: 'p-def', find: 'describes', replace: 'tells' }, { op: 'delete', id: 'missing' }];
    const res = await jinshu(['apply', file, tmp('bad.json', JSON.stringify(bad)), '--write']);
    assert.equal(res.code, 1);
    assert.match(res.err, /error NOT_FOUND op 1 #missing/);
    assert.match(res.err, /refused; nothing written/);
    assert.equal(readFileSync(file, 'utf8'), readFileSync(corpus('valid/prose.json'), 'utf8'));
  });

  it('refuses what a Markdown file cannot hold', async () => {
    const add = [{ op: 'insert', at: { end: 'doc' }, markup: '<details id="d"><summary>s</summary><p id="dp">x</p></details>' }];
    const res = await jinshu(['apply', corpus('valid/prose.md'), tmp('add.json', JSON.stringify(add)), '--json']);
    assert.equal(res.code, 1);
    assert.ok(JSON.parse(res.out).problems.some(p => p.code === 'FORMAT'));
  });
});

describe('jinshu usage', () => {
  it('exits 2 with a reason', async () => {
    for (const [args, reason] of [
      [[], /Usage: jinshu/],
      [['frobnicate'], /Unknown command "frobnicate"/],
      [['check'], /check takes 1 argument/],
      [['check', 'notes.txt'], /Cannot tell the format/],
      [['check', join(tmpdir(), 'jinshu-missing.json')], /Cannot read/],
      [['check', 'a.json', '--bogus'], /Unknown option --bogus/],
    ]) {
      const res = await jinshu(args);
      assert.equal(res.code, 2, String(args));
      assert.match(res.out + res.err, reason);
    }
  });

  it('--help exits 0', async () => {
    const res = await jinshu(['--help']);
    assert.equal(res.code, 0);
    assert.match(res.out, /check <file>/);
  });
});

describe('jinshu bin', () => {
  it('runs the TypeScript source through its own loader', () => {
    const ok = spawned(['check', corpus('valid/prose.json')]);
    assert.equal(ok.code, 0, ok.err);
    assert.match(ok.out, /prose\.json: ok/);
    const bad = spawned(['check', corpus('invalid/bad-tone.json'), '--json']);
    assert.equal(bad.code, 1);
    assert.equal(JSON.parse(bad.out).problems[0].code, 'INVALID');
  });

  it('applies ops from stdin and prints the document', () => {
    const res = spawned(['apply', corpus('valid/prose.json'), '-'], JSON.stringify([{ op: 'edit', id: 'p-def', find: 'describes', replace: 'tells' }]));
    assert.equal(res.code, 0, res.err);
    assert.equal(JSON.parse(res.out).blocks[1].runs[0].text, 'A limit tells where ');
  });
});
