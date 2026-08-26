#!/usr/bin/env node
// Build the changelog and release notes from the commit log.
//
// Two outputs from one source of truth, because they answer different
// questions:
//
//   --changelog  every user-visible commit, grouped. "What changed?"
//   --notes      only commits carrying a `Release-Note:` trailer, written for
//                a reader. "Should I care?"
//
// Nothing here is hand-maintained: a commit subject is its changelog entry, so
// the quality gate is code review, not a scramble on release day.
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const UNIT = '\x1f';
const RECORD = '\x1e';

/**
 * Who a change lands on. Grouping by commit type ("Features", "Bug Fixes")
 * tells a reader nothing — the same fix means different things to someone
 * writing an lxapp and someone embedding the SDK.
 */
export const AUDIENCES = [
  {
    id: 'lxapp',
    title: 'Writing an lxapp',
    scopes: [
      'lxapp', 'logic', 'page', 'bridge', 'react', 'vue', 'html', 'elements',
      'runtime', 'api', 'capability', 'webview', 'surface', 'splash', 'test',
      'update', 'upload', 'download', 'storage', 'i18n', 'theme',
    ],
  },
  {
    id: 'host',
    title: 'Embedding a host app',
    scopes: [
      'app', 'sdk', 'apple', 'ios', 'macos', 'android', 'windows', 'harmony',
      'shell', 'browser', 'terminal', 'desktop', 'platform', 'media', 'device-io',
    ],
  },
  { id: 'native', title: 'Rust native extensions', scopes: ['native', 'control', 'automation'] },
  { id: 'tooling', title: 'CLI and CI', scopes: ['cli', 'lxdev', 'devtool', 'ci', 'release', 'skill'] },
  { id: 'docs', title: 'Docs and examples', scopes: ['docs', 'showcase', 'examples', 'types'] },
];

const AUDIENCE_BY_SCOPE = new Map(
  AUDIENCES.flatMap((audience) => audience.scopes.map((scope) => [scope, audience.id])),
);

/** Types worth a changelog line on their own. */
const USER_VISIBLE = new Set(['feat', 'fix', 'perf', 'revert']);

const CONVENTIONAL = /^(?<type>[a-z]+)(?:\((?<scope>[^)]+)\))?(?<breaking>!)?: (?<subject>.+)$/;

export function parseCommit(raw) {
  const [hash = '', subjectLine = '', body = ''] = raw.split(UNIT);
  const match = CONVENTIONAL.exec(subjectLine.trim());
  const trailers = readTrailers(body);
  return {
    hash: hash.trim(),
    type: match?.groups?.type ?? null,
    scope: match?.groups?.scope ?? null,
    subject: match?.groups?.subject ?? subjectLine.trim(),
    breaking: Boolean(match?.groups?.breaking) || /^BREAKING CHANGE:/m.test(body),
    note: trailers.get('release-note') ?? null,
    audience: AUDIENCE_BY_SCOPE.get(match?.groups?.scope ?? '') ?? null,
  };
}

/** Git trailers, including the folded continuation lines a long note needs. */
function readTrailers(body) {
  const trailers = new Map();
  let key = null;
  let value = [];
  const flush = () => {
    if (key) trailers.set(key, value.join('\n').trim());
    key = null;
    value = [];
  };
  for (const line of body.split('\n')) {
    const start = /^([A-Za-z][A-Za-z-]*):[ \t]*(.*)$/.exec(line);
    if (start) {
      flush();
      key = start[1].toLowerCase();
      value = [start[2]];
      continue;
    }
    if (key && line.trim().length > 0) {
      value.push(line.trim());
      continue;
    }
    flush();
  }
  flush();
  return trailers;
}

export function readCommits(range, cwd = process.cwd()) {
  const out = execFileSync(
    'git',
    ['log', '--no-merges', `--format=%H${UNIT}%s${UNIT}%b${RECORD}`, range],
    { cwd, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
  );
  return out
    .split(RECORD)
    .map((entry) => entry.replace(/^\n/, ''))
    .filter((entry) => entry.trim().length > 0)
    .map(parseCommit);
}

function bySection(commits, keep) {
  const sections = new Map(AUDIENCES.map((audience) => [audience.id, []]));
  const other = [];
  for (const commit of commits.filter(keep)) {
    const bucket = commit.audience ? sections.get(commit.audience) : other;
    (bucket ?? other).push(commit);
  }
  return { sections, other };
}

function line(commit, { withScope }) {
  const scope = withScope && commit.scope ? `**${commit.scope}**: ` : '';
  const breaking = commit.breaking ? '**Breaking** — ' : '';
  return `- ${breaking}${scope}${commit.subject} (${commit.hash.slice(0, 9)})`;
}

export function renderChangelog(commits, { version, date }) {
  const { sections, other } = bySection(commits, (c) => USER_VISIBLE.has(c.type ?? '') || c.breaking);
  const parts = [`## ${version} — ${date}`, ''];

  const breaking = commits.filter((commit) => commit.breaking);
  if (breaking.length > 0) {
    // A 0.x release may break; saying so first is the whole point of the section.
    parts.push('### Breaking', '');
    parts.push(...breaking.map((commit) => line(commit, { withScope: true })), '');
  }

  for (const audience of AUDIENCES) {
    const entries = (sections.get(audience.id) ?? []).filter((commit) => !commit.breaking);
    if (entries.length === 0) continue;
    parts.push(`### ${audience.title}`, '');
    parts.push(...entries.map((commit) => line(commit, { withScope: true })), '');
  }

  const rest = other.filter((commit) => !commit.breaking);
  if (rest.length > 0) {
    parts.push('### Other', '');
    parts.push(...rest.map((commit) => line(commit, { withScope: true })), '');
  }
  if (parts.length === 2) parts.push('_No user-visible changes._', '');
  return parts.join('\n');
}

export function renderNotes(commits, { version }) {
  const noted = commits.filter((commit) => commit.note);
  const { sections, other } = bySection(noted, () => true);
  const parts = [`# LingXia ${version}`, ''];

  const breaking = noted.filter((commit) => commit.breaking);
  if (breaking.length > 0) {
    parts.push('## Breaking changes', '');
    for (const commit of breaking) parts.push(`### ${commit.subject}`, '', commit.note, '');
  }

  for (const audience of AUDIENCES) {
    const entries = (sections.get(audience.id) ?? []).filter((commit) => !commit.breaking);
    if (entries.length === 0) continue;
    parts.push(`## ${audience.title}`, '');
    for (const commit of entries) parts.push(`### ${commit.subject}`, '', commit.note, '');
  }
  for (const commit of other.filter((commit) => !commit.breaking)) {
    parts.push(`### ${commit.subject}`, '', commit.note, '');
  }

  if (noted.length === 0) {
    parts.push(
      '_No commit in this range carried a `Release-Note:` trailer._',
      '',
      'Add one to any change a reader should know about:',
      '',
      '```',
      'feat(lxapp): add lx.surface.openUrl',
      '',
      'Release-Note: lxapps can now open an external URL in its own tab,',
      'subject to trustedDomains.',
      '```',
      '',
    );
  }
  return parts.join('\n');
}

/**
 * The last release tag reachable from `ref`. Releases are tagged per artifact
 * and in lockstep (`lingxia-cli-v0.11.2`, `lingxia-types-v0.11.2`, …), so any
 * of them marks the same release; `describe` walks back through history, which
 * is what "since the last release" means and does not depend on tag dates or
 * on how the names sort.
 *
 * Returns null on a repo that has never released. The caller must not fall
 * back to the whole history — that printed 3138 entries here, which is not a
 * changelog.
 */
export function lastReleaseTag(cwd = process.cwd(), ref = 'HEAD') {
  try {
    return execFileSync(
      'git',
      ['describe', '--tags', '--abbrev=0', '--match=*-v[0-9]*', ref],
      { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] },
    ).trim() || null;
  } catch {
    return null;
  }
}

function main(argv) {
  const mode = argv.includes('--notes') ? 'notes' : 'changelog';
  const at = (flag, fallback) => {
    const index = argv.indexOf(flag);
    return index === -1 ? fallback : argv[index + 1];
  };
  const to = at('--to', 'HEAD');
  const from = at('--from', lastReleaseTag());
  if (!from) {
    process.stderr.write(
      [
        'No release tag found, so there is no range to describe.',
        '',
        'Pass an explicit start:',
        '  scripts/release/main.sh changelog --from <tag-or-commit>',
        '',
        'Defaulting to the whole history would print every commit ever made,',
        'which is not a changelog.',
        '',
      ].join('\n'),
    );
    return 2;
  }
  const range = `${from}..${to}`;
  const version = at('--version', 'Unreleased');
  const date = at('--date', new Date().toISOString().slice(0, 10));
  const commits = readCommits(range);
  process.stdout.write(
    mode === 'notes' ? renderNotes(commits, { version }) : renderChangelog(commits, { version, date }),
  );
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  process.exit(main(process.argv.slice(2)));
}
