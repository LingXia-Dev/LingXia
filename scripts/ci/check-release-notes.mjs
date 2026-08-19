#!/usr/bin/env node
// Fail a change to the public lx API that ships without a release note.
//
// `packages/lingxia-types/src/testing/public-api.ts` is not a hand-kept list:
// 51 compile-time assertions tie it to the real types, so a public API cannot
// be added or removed without touching it. That makes it an exact signal —
// nobody has to remember to declare "this is public".
//
// The note is required on the range, not on every commit: a refactor may span
// several commits and still be one thing worth telling a reader about.
import { execFileSync } from 'node:child_process';
import process from 'node:process';
import { readCommits } from '../release/changelog.mjs';

export const PUBLIC_SURFACE = 'packages/lingxia-types/src/testing/public-api.ts';

/**
 * The commit both refs share. `base..HEAD` is only the PR's own commits when
 * that commit is present; on the shallow clone CI checks out it is not, and
 * the range silently widens to most of the history — which made this gate
 * accuse a PR of editing an API it never touched.
 */
export function mergeBase(base, head, cwd = process.cwd()) {
  try {
    return execFileSync('git', ['merge-base', base, head], {
      cwd,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim() || null;
  } catch {
    return null;
  }
}

export function touchedBy(range, file, cwd = process.cwd()) {
  const out = execFileSync('git', ['log', '--no-merges', '--format=%H', '--name-only', range, '--', file], {
    cwd,
    encoding: 'utf8',
  });
  return out
    .split('\n')
    .filter((line) => /^[0-9a-f]{40}$/.test(line.trim()))
    .map((line) => line.trim());
}

function main(argv) {
  const base = argv[0] ?? process.env.GITHUB_BASE_REF ?? 'origin/main';
  const head = argv[1] ?? 'HEAD';

  const shared = mergeBase(base, head);
  if (!shared) {
    process.stderr.write(
      [
        `Cannot find a commit shared by ${base} and ${head}, so there is no`,
        'range to inspect. A shallow clone is the usual cause — deepen it',
        'until the merge base is present:',
        '',
        `  git fetch --no-tags --unshallow origin ${base.replace(/^origin\//, '')}`,
        '',
        'Refusing to guess: a widened range makes this gate blame a change it',
        'never saw.',
        '',
      ].join('\n'),
    );
    return 2;
  }
  const range = `${shared}..${head}`;

  const touching = touchedBy(range, PUBLIC_SURFACE);
  if (touching.length === 0) {
    process.stdout.write('release notes: public lx API unchanged, no note required\n');
    return 0;
  }

  const commits = readCommits(range);
  const noted = commits.filter((commit) => commit.note);
  if (noted.length > 0) {
    process.stdout.write(
      `release notes: ${touching.length} commit(s) changed the public lx API, ` +
        `${noted.length} carry a Release-Note trailer\n`,
    );
    return 0;
  }

  process.stderr.write(
    [
      `This change edits the public lx API (${PUBLIC_SURFACE}) but no commit in`,
      `${range} carries a Release-Note trailer, so it would ship with a`,
      'changelog line and nothing a reader can act on.',
      '',
      'Commits that changed the public surface:',
      ...touching.map((hash) => `  ${hash.slice(0, 9)}`),
      '',
      'Add a trailer to one of them — what changed, and what a caller does now:',
      '',
      '  feat(lxapp): add lx.surface.openUrl',
      '',
      '  Release-Note: lxapps can open an external URL in its own tab,',
      '  subject to trustedDomains.',
      '',
    ].join('\n'),
  );
  return 1;
}

if (import.meta.url === `file://${process.argv[1]}`) process.exit(main(process.argv.slice(2)));
export { main };
