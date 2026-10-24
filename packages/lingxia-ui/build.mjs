import { readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { buildPages } from './compiler/index.mjs';

// Builds a Logic-free lxapp whose pages are all GPU Views; `lingxia build` covers the rest.
const root = resolve(process.argv[2] ?? '.');
const manifest = JSON.parse(await readFile(join(root, 'lxapp.json'), 'utf8'));
if (manifest.logic !== false) throw Error('Use lingxia build to compile pages with Logic');
await buildPages(root, manifest.pages.map(page => `${page.path}.tsx`), join(root, 'dist'));
const pages = manifest.pages.map(page => ({ ...page, path: `${page.path}.js` }));
await writeFile(join(root, 'dist/lxapp.json'), JSON.stringify({ ...manifest, pages }, null, 2) + '\n');
