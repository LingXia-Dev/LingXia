import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { PUBLIC_SURFACE, mergeBase, touchedBy } from './check-release-notes.mjs';

/** A throwaway repo, so the gate is exercised against real git plumbing. */
function repo() {
  const dir = mkdtempSync(path.join(tmpdir(), 'lx-notes-'));
  const git = (...args) => execFileSync('git', args, { cwd: dir, encoding: 'utf8' });
  git('init', '-q', '-b', 'main');
  git('config', 'user.email', 'test@example.com');
  git('config', 'user.name', 'Test');
  writeFileSync(path.join(dir, 'seed.txt'), 'seed\n');
  git('add', '.');
  git('commit', '-qm', 'chore: seed');
  return { dir, git };
}

function commitPublicSurface({ dir, git }, message) {
  mkdirSync(path.join(dir, path.dirname(PUBLIC_SURFACE)), { recursive: true });
  writeFileSync(path.join(dir, PUBLIC_SURFACE), `export const X = ${Date.now()};\n`);
  git('add', '.');
  git('commit', '-qm', message);
}

test('sees a commit that edits the public surface', () => {
  const r = repo();
  const base = r.git('rev-parse', 'HEAD').trim();
  commitPublicSurface(r, 'feat(lxapp): add an api');

  assert.equal(touchedBy(`${base}..HEAD`, PUBLIC_SURFACE, r.dir).length, 1);
});

test('ignores a change that leaves the public surface alone', () => {
  const r = repo();
  const base = r.git('rev-parse', 'HEAD').trim();
  writeFileSync(path.join(r.dir, 'seed.txt'), 'changed\n');
  r.git('add', '.');
  r.git('commit', '-qm', 'fix(cli): tidy');

  assert.deepEqual(touchedBy(`${base}..HEAD`, PUBLIC_SURFACE, r.dir), []);
});

test('rejects an unnoted breaking commit outside the typed lx API', () => {
  const r = repo();
  const base = r.git('rev-parse', 'HEAD').trim();
  writeFileSync(path.join(r.dir, 'seed.txt'), 'breaking\n');
  r.git('add', '.');
  r.git('commit', '-qm', 'feat(cli)!: remove the old command');

  const script = path.resolve('scripts/ci/check-release-notes.mjs');
  assert.throws(
    () => execFileSync(process.execPath, [script, base, 'HEAD'], {
      cwd: r.dir,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }),
    (error) => {
      assert.equal(error.status, 1);
      assert.match(error.stderr, /Every breaking commit/);
      assert.match(error.stderr, /remove the old command/);
      return true;
    },
  );
});

test('a rename of the file still counts as touching it', () => {
  const r = repo();
  commitPublicSurface(r, 'feat(lxapp): add an api');
  const base = r.git('rev-parse', 'HEAD').trim();
  r.git('rm', '-q', PUBLIC_SURFACE);
  r.git('commit', '-qm', 'refactor(types): drop the manifest');

  assert.equal(touchedBy(`${base}..HEAD`, PUBLIC_SURFACE, r.dir).length, 1);
});

test('a shallow clone is refused, not blamed', () => {
  const r = repo();
  // A public-API commit that is already on the base — a PR must not be
  // charged for it.
  commitPublicSurface(r, 'feat(lxapp): an api that shipped long ago');
  for (let i = 0; i < 5; i += 1) {
    writeFileSync(path.join(r.dir, 'seed.txt'), `filler ${i}\n`);
    r.git('add', '.');
    r.git('commit', '-qm', `chore: filler ${i}`);
  }
  // main and the branch must actually diverge, or a depth-1 clone of the
  // branch already contains the base and there is nothing to reproduce.
  r.git('branch', 'feature');
  writeFileSync(path.join(r.dir, 'seed.txt'), 'main moves on\n');
  r.git('add', '.');
  r.git('commit', '-qm', 'chore: main moves on');
  r.git('checkout', '-q', 'feature');
  writeFileSync(path.join(r.dir, 'seed.txt'), 'the actual change\n');
  r.git('add', '.');
  r.git('commit', '-qm', 'fix(cli): tidy');
  r.git('checkout', '-q', 'main');

  // Full history: the merge base is the branch point, and the public-API
  // commit sits before it, so nothing is required.
  assert.ok(mergeBase('main', 'feature', r.dir));
  assert.deepEqual(touchedBy(`${mergeBase('main', 'feature', r.dir)}..feature`, PUBLIC_SURFACE, r.dir), []);

  // What CI has: the feature branch checked out shallowly, with the base
  // fetched shallowly too, so the branch point is in neither.
  const shallow = mkdtempSync(path.join(tmpdir(), 'lx-shallow-'));
  execFileSync(
    'git',
    ['clone', '-q', '--depth', '1', '--branch', 'feature', `file://${r.dir}`, shallow],
    { encoding: 'utf8' },
  );
  execFileSync('git', ['fetch', '-q', '--no-tags', '--depth', '1', 'origin', 'main:refs/remotes/origin/main'], {
    cwd: shallow,
    encoding: 'utf8',
  });
  assert.equal(mergeBase('origin/main', 'HEAD', shallow), null, 'expected no shared commit in a shallow clone');

  const script = path.resolve('scripts/ci/check-release-notes.mjs');
  assert.throws(
    () => execFileSync(process.execPath, [script, 'origin/main', 'HEAD'], {
      cwd: shallow,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }),
    (error) => {
      assert.equal(error.status, 2);
      // The old failure named the wrong culprit; this one names the cause.
      assert.match(error.stderr, /Cannot find a commit shared by/);
      assert.doesNotMatch(error.stderr, /edits the public lx API/);
      return true;
    },
  );
});
