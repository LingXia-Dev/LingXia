// Keep src/version.ts in step with package.json so the runtime never reports a
// version the package has already moved past (lxdev compares the two).
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../', import.meta.url);
const { version } = JSON.parse(readFileSync(new URL('package.json', root), 'utf8'));
const target = fileURLToPath(new URL('src/version.ts', root));
const source = readFileSync(target, 'utf8');
const updated = source.replace(/export const VERSION = "[^"]*";/, `export const VERSION = "${version}";`);
if (!updated.includes(`export const VERSION = "${version}";`)) {
  throw new Error('src/version.ts has no VERSION line to update');
}
if (updated !== source) writeFileSync(target, updated);
