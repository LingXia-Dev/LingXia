#!/usr/bin/env node
// Assembles the gallery under one of Liuli's theme packs, as its own lxapp, outside the repository:
//   node scripts/gallery-with-pack.mjs <pack> [--out <dir>]
// then `lingxia build` or `lingxia dev listen` in the directory it prints. A pack belongs to an
// lxapp (theme/pack.json) and is fixed when the app is built, so a second pack means a second app.
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const liuli = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const packs = join(liuli, 'themes');
const [name, ...rest] = process.argv.slice(2);
const known = existsSync(packs) ? readdirSync(packs).filter(each => existsSync(join(packs, each, 'pack.json'))) : [];
if (!name || !known.includes(name)) {
  console.error(`usage: gallery-with-pack.mjs <pack> [--out <dir>]\npacks: ${known.join(', ') || 'none'}`);
  process.exit(2);
}
const outFlag = rest.indexOf('--out');
const out = resolve(outFlag >= 0 && rest[outFlag + 1] ? rest[outFlag + 1] : join(tmpdir(), `liuli-gallery-${name}`));
const pack = JSON.parse(readFileSync(join(packs, name, 'pack.json'), 'utf8'));

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
// The gallery's own files are copied; build output, sessions and device specs stay behind.
const skipped = new Set(['dist', 'node_modules', '.lingxia', '.baseline', 'test-results', 'zz-tests']);
cpSync(join(liuli, 'gallery'), join(out, 'gallery'), {
  recursive: true,
  filter: source => !skipped.has(source.slice(join(liuli, 'gallery').length + 1).split('/')[0]) && !/\/\.dist[^/]*$/.test(source),
});
// The packages it compiles from are linked, at the places the gallery's imports expect them.
for (const each of ['core', 'charts']) symlinkSync(join(liuli, each), join(out, each), 'dir');
symlinkSync(resolve(liuli, '..', 'node_modules'), join(out, 'node_modules'), 'dir');

const gallery = join(out, 'gallery');
// The baseline's capture spec runs in the copy; its type paths point back into this repository.
mkdirSync(join(gallery, 'zz-tests'), { recursive: true });
cpSync(join(liuli, 'gallery', 'zz-tests', 'baseline.test.ts'), join(gallery, 'zz-tests', 'baseline.test.ts'));
const specConfig = JSON.parse(readFileSync(join(liuli, 'gallery', 'zz-tests', 'tsconfig.json'), 'utf8'));
specConfig.compilerOptions.paths = { '@lingxia/test': [resolve(liuli, '..', 'lingxia-test', 'dist', 'index.d.ts')] };
writeFileSync(join(gallery, 'zz-tests', 'tsconfig.json'), `${JSON.stringify(specConfig, null, 2)}\n`);
mkdirSync(join(gallery, 'theme'), { recursive: true });
writeFileSync(join(gallery, 'theme', 'pack.json'), `${JSON.stringify(pack, null, 2)}\n`);
const manifestPath = join(gallery, 'lxapp.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
manifest.appId = `${manifest.appId}-${name}`;
manifest.appName = `${manifest.appName} (${pack.name})`;
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(gallery);
