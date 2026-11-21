import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

for (const status of ['failed', 'timeout', 'xpass']) {
  test(`covers rejects a complete run containing ${status}`, () => {
    const dir = mkdtempSync(join(tmpdir(), 'showcase-covers-'));
    try {
      const report = join(dir, 'report.json');
      writeFileSync(report, JSON.stringify({total:1, partial:false, filtered:false, cases:[{status}]}));
      const result = spawnSync(process.execPath, [fileURLToPath(new URL('../scripts/check-covers.mjs', import.meta.url)), report], {encoding:'utf8'});
      assert.equal(result.status, 2, result.stderr);
      assert.match(result.stderr, /empty or failing run/);
    } finally {
      rmSync(dir, {recursive:true, force:true});
    }
  });
}
