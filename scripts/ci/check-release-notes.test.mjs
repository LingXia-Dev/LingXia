import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { PUBLIC_SURFACE, touchedBy } from './check-release-notes.mjs';

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

test('a rename of the file still counts as touching it', () => {
  const r = repo();
  commitPublicSurface(r, 'feat(lxapp): add an api');
  const base = r.git('rev-parse', 'HEAD').trim();
  r.git('rm', '-q', PUBLIC_SURFACE);
  r.git('commit', '-qm', 'refactor(types): drop the manifest');

  assert.equal(touchedBy(`${base}..HEAD`, PUBLIC_SURFACE, r.dir).length, 1);
});
