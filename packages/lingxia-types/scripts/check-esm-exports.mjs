// Guards the packaging bug that made `import { … } from '@lingxia/types'`
// resolve to a CommonJS file: the lxapp Logic bundler inlines the `import`
// target verbatim, so a `require(...)` in it throws at module-eval time and
// takes the whole Logic layer — every Page() registration — down with it.
//
// Every `import` condition must therefore point at a file that is real ESM and
// is *seen* as ESM (`.mjs`, or a `.js` under a `"type": "module"` scope).
import { readFile } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = fileURLToPath(new URL('..', import.meta.url));
const manifest = JSON.parse(await readFile(join(packageRoot, 'package.json'), 'utf8'));

const CJS_MARKERS = [
  /\brequire\s*\(/,
  /\bmodule\.exports\b/,
  /\bexports\.[A-Za-z_$]/,
  /Object\.defineProperty\(exports\b/,
];

/** Node's module-format rule: extension first, then the nearest package scope. */
function moduleFormat(filePath) {
  if (filePath.endsWith('.mjs')) return 'module';
  if (filePath.endsWith('.cjs')) return 'commonjs';
  for (let dir = dirname(filePath); ; dir = dirname(dir)) {
    const candidate = join(dir, 'package.json');
    if (existsSync(candidate)) {
      const scope = JSON.parse(readFileSync(candidate, 'utf8'));
      return scope?.type === 'module' ? 'module' : 'commonjs';
    }
    const parent = dirname(dir);
    if (parent === dir) return 'commonjs';
  }
}

const failures = [];
for (const [subpath, entry] of Object.entries(manifest.exports)) {
  const target = entry?.import;
  if (!target) continue;
  const filePath = resolve(packageRoot, target);
  if (!existsSync(filePath)) {
    failures.push(`${subpath}: import condition ${target} does not exist (run npm run build)`);
    continue;
  }
  const format = moduleFormat(filePath);
  if (format !== 'module') {
    failures.push(`${subpath}: import condition ${target} is resolved as CommonJS`);
  }
  const source = await readFile(filePath, 'utf8');
  const marker = CJS_MARKERS.find((pattern) => pattern.test(source));
  if (marker) {
    failures.push(`${subpath}: import condition ${target} contains CommonJS syntax (${marker})`);
  }
}

if (failures.length > 0) {
  console.error('ESM export check failed:');
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(`ESM export check passed for ${Object.keys(manifest.exports).length} subpaths.`);
