import { readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { buildPages } from './compiler.mjs';
const root = resolve(process.argv[2] ?? '.');
const manifest = JSON.parse(await readFile(join(root, 'lxapp.json'), 'utf8'));
if (manifest.logic !== false) throw Error('Use lingxia build to compile pages with Logic');
await buildPages(root, manifest.pages.map(page => `${page.path}.ui.tsx`), join(root, 'dist'));
await writeFile(join(root, 'dist/lxapp.json'), JSON.stringify(manifest, null, 2) + '\n');
