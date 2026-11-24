// Write src/package-version.ts from package.json (untracked): lxdev compares the
// runtime's version with the package's, so they must never drift.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../', import.meta.url);
const { version } = JSON.parse(readFileSync(new URL('package.json', root), 'utf8'));
const target = fileURLToPath(new URL('src/package-version.ts', root));
const source = `// Generated from package.json by scripts/write-version.mjs; not tracked.\nexport const VERSION = ${JSON.stringify(version)};\n`;
if (!existsSync(target) || readFileSync(target, 'utf8') !== source) writeFileSync(target, source);
