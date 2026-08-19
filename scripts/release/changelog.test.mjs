import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { lastReleaseTag, parseCommit, renderChangelog, renderNotes } from './changelog.mjs';

/** A throwaway repo, so tag discovery is exercised against real git. */
function repo() {
  const dir = mkdtempSync(path.join(tmpdir(), 'lx-changelog-'));
  const git = (...args) => execFileSync('git', args, { cwd: dir, encoding: 'utf8' });
  git('init', '-q', '-b', 'main');
  git('config', 'user.email', 'test@example.com');
  git('config', 'user.name', 'Test');
  writeFileSync(path.join(dir, 'seed.txt'), 'seed\n');
  git('add', '.');
  git('commit', '-qm', 'chore: seed');
  return { dir, git };
}

const UNIT = '\x1f';
const commit = (subject, body = '') => parseCommit(`abcdef1234${UNIT}${subject}${UNIT}${body}`);

test('reads type, scope, breaking marker and audience from a subject', () => {
  const feat = commit('feat(lxapp): add lx.surface.openUrl');
  assert.equal(feat.type, 'feat');
  assert.equal(feat.scope, 'lxapp');
  assert.equal(feat.audience, 'lxapp');
  assert.equal(feat.breaking, false);

  assert.equal(commit('fix(windows): stop the crash').audience, 'host');
  assert.equal(commit('feat(cli): add a flag').audience, 'tooling');
  // An unknown scope must not be dropped; it lands in "Other".
  assert.equal(commit('fix(quantum): something new').audience, null);
});

test('marks a breaking change from either the ! or the body', () => {
  assert.equal(commit('feat(lxapp)!: drop the old API').breaking, true);
  assert.equal(commit('feat(lxapp): rework', 'BREAKING CHANGE: renamed').breaking, true);
});

test('keeps a multi-line Release-Note trailer intact', () => {
  const note = commit(
    'feat(lxapp): add openUrl',
    'Some body.\n\nRelease-Note: lxapps can open an external URL.\nIt is subject to trustedDomains.\n',
  );
  assert.equal(note.note, 'lxapps can open an external URL.\nIt is subject to trustedDomains.');
});

test('does not mistake a prose colon for a trailer', () => {
  const note = commit('fix(cli): tidy', 'We had a problem: the path was wrong.\n');
  assert.equal(note.note, null);
});

test('a non-conventional subject still appears, unclassified', () => {
  const odd = commit('merge branch whatever');
  assert.equal(odd.type, null);
  assert.equal(odd.subject, 'merge branch whatever');
});

test('the changelog leads with breaking changes and groups by audience', () => {
  const out = renderChangelog(
    [
      commit('feat(lxapp)!: drop lx.surface.open'),
      commit('fix(windows): stop the crash'),
      commit('feat(lxapp): add openUrl'),
      commit('chore(ci): bump an action'),
      commit('fix(quantum): odd scope'),
    ],
    { version: '0.12.0', date: '2026-01-01' },
  );

  assert.match(out, /^## 0\.12\.0 — 2026-01-01/);
  assert.ok(out.indexOf('### Breaking') < out.indexOf('### Writing an lxapp'));
  assert.match(out, /\*\*Breaking\*\* — \*\*lxapp\*\*: drop lx\.surface\.open/);
  assert.match(out, /### Embedding a host app/);
  assert.match(out, /### Other/);
  // Housekeeping is not a changelog entry.
  assert.doesNotMatch(out, /bump an action/);
  // A breaking change appears once, in its own section.
  assert.equal(out.split('drop lx.surface.open').length - 1, 1);
});

test('an empty range says so rather than rendering an empty section', () => {
  const out = renderChangelog([commit('chore(ci): bump')], { version: '0.12.0', date: '2026-01-01' });
  assert.match(out, /_No user-visible changes\._/);
});

test('release notes carry only what an author wrote a note for', () => {
  const out = renderNotes(
    [
      commit('feat(lxapp): add openUrl', 'Release-Note: Open an external URL in its own tab.'),
      commit('fix(windows): stop the crash'),
    ],
    { version: '0.12.0' },
  );

  assert.match(out, /Open an external URL in its own tab\./);
  // No trailer, no prose: the changelog already lists it.
  assert.doesNotMatch(out, /stop the crash/);
});

test('release notes explain the trailer when nobody used one', () => {
  const out = renderNotes([commit('fix(cli): tidy')], { version: '0.12.0' });
  assert.match(out, /No commit in this range carried a `Release-Note:` trailer/);
  assert.match(out, /Release-Note: lxapps can now open/);
});

test('finds the last release tag reachable from HEAD', () => {
  const r = repo();
  // Releases are tagged per artifact in lockstep, never as a bare `v0.11.2`.
  r.git('tag', 'lingxia-types-v0.11.1');
  r.git('tag', 'lingxia-cli-v0.11.1');
  writeFileSync(path.join(r.dir, 'seed.txt'), 'next\n');
  r.git('add', '.');
  r.git('commit', '-qm', 'feat(cli): released later');
  r.git('tag', 'lingxia-cli-v0.11.2');
  writeFileSync(path.join(r.dir, 'seed.txt'), 'unreleased\n');
  r.git('add', '.');
  r.git('commit', '-qm', 'feat(cli): not yet released');

  // The nearest release, so the range covers only what shipped since.
  assert.equal(lastReleaseTag(r.dir), 'lingxia-cli-v0.11.2');

  const out = execFileSync(
    process.execPath,
    [path.resolve('scripts/release/changelog.mjs'), '--version', 'TEST'],
    { cwd: r.dir, encoding: 'utf8' },
  );
  assert.match(out, /not yet released/);
  assert.doesNotMatch(out, /released later/);
});

test('reports no tag rather than guessing on a repo that never released', () => {
  const r = repo();
  r.git('tag', 'backup-preclean');
  // A non-release tag must not be mistaken for one.
  assert.equal(lastReleaseTag(r.dir), null);
});

test('refuses to describe the whole history when there is no tag', () => {
  const r = repo();
  const script = path.resolve('scripts/release/changelog.mjs');

  // Defaulting to the full history printed 3138 entries on this repo, because
  // no tag matched and the range silently became `HEAD`.
  assert.throws(
    () => execFileSync(process.execPath, [script, '--version', 'TEST'], {
      cwd: r.dir,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }),
    (error) => {
      assert.equal(error.status, 2);
      assert.match(error.stderr, /No release tag found/);
      return true;
    },
  );

  // An explicit start still works on the same repo.
  const first = execFileSync('git', ['rev-list', '--max-parents=0', 'HEAD'], {
    cwd: r.dir,
    encoding: 'utf8',
  }).trim();
  const out = execFileSync(process.execPath, [script, '--from', first, '--version', 'TEST'], {
    cwd: r.dir,
    encoding: 'utf8',
  });
  assert.match(out, /## TEST/);
});
