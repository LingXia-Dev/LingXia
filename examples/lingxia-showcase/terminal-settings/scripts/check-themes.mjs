import { readFile, readdir } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const { PACKAGED } = await import(join(root, 'pages/settings/schemes.ts'));
const requiredColors = [
  'background', 'foreground', 'black', 'red', 'green', 'yellow', 'blue',
  'purple', 'cyan', 'white', 'brightBlack', 'brightRed', 'brightGreen',
  'brightYellow', 'brightBlue', 'brightPurple', 'brightCyan', 'brightWhite',
];
const licensed = (await readdir(join(root, 'themes'), { withFileTypes: true }))
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();
const packaged = PACKAGED.map((theme) => theme.name).sort();

if (JSON.stringify(licensed) !== JSON.stringify(packaged)) {
  throw new Error(`themes/ (${licensed.join(', ')}) and the packaged schemes (${packaged.join(', ')}) differ`);
}
for (const theme of PACKAGED) {
  if (!theme.displayName || !theme.author || !theme.upstream || !theme.spdx) {
    throw new Error(`${theme.name}: needs displayName, author, upstream and spdx`);
  }
  for (const key of requiredColors) {
    if (typeof theme.scheme[key] !== 'string' || !/^#[0-9a-f]{6}$/i.test(theme.scheme[key])) {
      throw new Error(`${theme.name}: no valid ${key}`);
    }
  }
  const license = await readFile(join(root, 'themes', theme.name, 'LICENSE'), 'utf8');
  if (!license.trim()) throw new Error(`${theme.name}: LICENSE is empty`);
}

console.log(`verified ${PACKAGED.length} packaged terminal themes`);
