import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Parity is asserted against the committed React showcase, so uncommitted work there cannot fail
// these tests; drift is reported instead. Outside a git checkout the working tree is the reference.
const root = fileURLToPath(new URL('../../lingxia-showcase/lxapp/', import.meta.url));
const git = (...args) => spawnSync('git', args, { cwd: root, encoding: 'utf8' });
const tracked = git('rev-parse', '--is-inside-work-tree').stdout.trim() === 'true';
const reported = new Set();

export function readReference(path) {
  if (!tracked) return readFileSync(join(root, path), 'utf8');
  const committed = git('show', `HEAD:./${path}`);
  if (committed.status !== 0) throw Error(`React reference ${path} is not committed: ${committed.stderr.trim()}`);
  if (!reported.has(path) && git('diff', '--quiet', 'HEAD', '--', path).status !== 0) {
    reported.add(path);
    console.warn(`notice: examples/lingxia-showcase/lxapp/${path} differs from HEAD; parity is checked against the committed file`);
  }
  return committed.stdout;
}
