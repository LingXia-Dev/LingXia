import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, relative, isAbsolute } from 'node:path';

const found = new Map();

/** The nearest package.json above `file` that names a package: `{ dir, name }`, or undefined. */
export function packageOf(file) {
  let dir = dirname(file);
  const seen = [];
  for (;;) {
    if (found.has(dir)) break;
    seen.push(dir);
    const manifest = join(dir, 'package.json');
    if (existsSync(manifest)) {
      let name;
      try { name = JSON.parse(readFileSync(manifest, 'utf8')).name; } catch {}
      if (typeof name === 'string' && name) { found.set(dir, { dir, name }); break; }
    }
    const up = dirname(dir);
    if (up === dir) { found.set(dir, undefined); break; }
    dir = up;
  }
  const result = found.get(dir);
  for (const each of seen) found.set(each, result);
  return result;
}

const posix = path => path.split('\\').join('/');

/** Whether `file` is the lxapp's own source: under `root` and not in a node_modules. */
export function authored(root, file) {
  const rel = relative(root, file);
  return !rel.startsWith('..') && !isAbsolute(rel) && !posix(rel).split('/').includes('node_modules');
}

/**
 * How a file is named in ids and source positions: the lxapp's own files by their path in the
 * lxapp; another package's by its package name and path inside it, the same whether it is a
 * workspace link or an installed copy.
 */
export function sourceLabel(root, file) {
  if (authored(root, file)) return posix(relative(root, file));
  const owner = packageOf(file);
  return owner ? `${owner.name}/${posix(relative(owner.dir, file))}` : posix(relative(root, file));
}
