// Writes src/i18n/words.ts from messages/<lang>.json with `lingxia gen i18n`. `--check` writes
// nothing and fails on drift. The CLI generates package words only from the repo's i18n/liuli
// scope under the `@liuli.` prefix, so this hands it a copy of the repo's scopes with jinshu's
// catalogue standing in for liuli's, keyed `@liuli.`, and puts `@jinshu.` back in its output.
// Run: node scripts/words.mjs [--check]. The CLI is LINGXIA, else the repo's newest built one, else PATH.
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const repoI18n = join(root, '../../../i18n');
const out = join(root, 'src/i18n/words.ts');
const check = process.argv.includes('--check');

// A CLI on PATH can be older than the repo's flags; the one built here matches them.
function repoCli() {
  const exe = process.platform === 'win32' ? 'lingxia.exe' : 'lingxia';
  const built = ['debug', 'release']
    .map(profile => join(root, '../../../target', profile, exe))
    .filter(path => existsSync(path))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return built[0];
}

const temp = mkdtempSync(join(tmpdir(), 'jinshu-words-'));
try {
  const input = join(temp, 'i18n');
  mkdirSync(join(input, 'liuli'), { recursive: true });
  // Copied, not linked: a symlink needs extra rights on Windows.
  for (const scope of readdirSync(repoI18n)) if (scope !== 'liuli' && !scope.includes('.')) cpSync(join(repoI18n, scope), join(input, scope), { recursive: true });
  for (const file of readdirSync(join(root, 'messages')).filter(name => name.endsWith('.json'))) {
    const catalogue = JSON.parse(readFileSync(join(root, 'messages', file), 'utf8'));
    const renamed = Object.fromEntries(Object.entries(catalogue).map(([key, entry]) => {
      if (!key.startsWith('@jinshu.')) throw new Error(`messages/${file}: '${key}' is not a @jinshu. key`);
      return [`@liuli.${key.slice('@jinshu.'.length)}`, entry];
    }));
    writeFileSync(join(input, 'liuli', file), JSON.stringify(renamed, null, 2));
  }
  const generated = join(temp, 'words.ts');
  const cli = process.env.LINGXIA ?? repoCli() ?? 'lingxia';
  try {
    execFileSync(cli, ['gen', 'i18n', '-i', input, '--liuli-out', generated, '--no-rust', '--no-android', '--no-ios', '--no-harmony', '--no-view'], { stdio: ['ignore', 'ignore', 'inherit'] });
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    console.error(`words: the \`${cli}\` CLI was not found; build it with \`cargo build -p lingxia-cli\`, put it on PATH or name it with LINGXIA=/path/to/lingxia`);
    process.exit(1);
  }
  const words = readFileSync(generated, 'utf8')
    .replace('from i18n/liuli/. Do not edit.', 'from messages/ by scripts/words.mjs. Do not edit.')
    .replace(/^ {2}"@liuli\./gm, '  "@jinshu.');
  if (!check) writeFileSync(out, words);
  else if (readFileSync(out, 'utf8') !== words) {
    console.error('src/i18n/words.ts is stale; run `node scripts/words.mjs`');
    process.exitCode = 1;
  }
} finally {
  rmSync(temp, { recursive: true, force: true });
}
