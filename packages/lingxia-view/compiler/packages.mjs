import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, relative, isAbsolute } from 'node:path';

const found = new Map();

/** The nearest package.json above `file` that names a package: `{ dir, name, tokens }`, or undefined.
 * `tokens` is the package's declared token stylesheet (`"lingxia": { "tokens": "./tokens.css" }`). */
export function packageOf(file) {
  let dir = dirname(file);
  const seen = [];
  for (;;) {
    if (found.has(dir)) break;
    seen.push(dir);
    const manifest = join(dir, 'package.json');
    if (existsSync(manifest)) {
      let json;
      try { json = JSON.parse(readFileSync(manifest, 'utf8')); } catch {}
      const name = json?.name;
      if (typeof name === 'string' && name) { found.set(dir, { dir, name, tokens: json.lingxia?.tokens }); break; }
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

function readPackage(dir) {
  let json;
  try { json = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')); } catch { return undefined; }
  if (typeof json?.name !== 'string' || !json.name) return undefined;
  const peers = Object.keys(json.peerDependencies ?? {}).sort();
  const optionalPeers = new Set(Object.entries(json.peerDependenciesMeta ?? {})
    .filter(([, meta]) => meta?.optional)
    .map(([name]) => name));
  return { dir, name: json.name, tokens: json.lingxia?.tokens, peers, optionalPeers };
}

function findInstalled(name, fromDirs) {
  const parts = name.split('/');
  for (const start of fromDirs) {
    let dir = start;
    for (;;) {
      const candidate = join(dir, 'node_modules', ...parts);
      if (existsSync(join(candidate, 'package.json'))) return readPackage(candidate);
      const up = dirname(dir);
      if (up === dir) break;
      dir = up;
    }
  }
  return undefined;
}

/**
 * Expand `bundled` packages transitively across `peerDependencies`, returning a map of every
 * package that declares `lingxia.tokens`. Missing non-optional peers are recorded in `problems`.
 */
export function peerTokenPackages(root, bundled, problems) {
  const visited = new Map();
  const queue = [...bundled.values()].sort((a, b) => a.name < b.name ? -1 : 1);
  for (const pkg of queue) visited.set(pkg.name, readPackage(pkg.dir) ?? { ...pkg, peers: [], optionalPeers: new Set() });
  const order = [...visited.values()];
  for (let i = 0; i < order.length; i++) {
    const pkg = order[i];
    for (const peerName of pkg.peers) {
      // @lingxia/view is always supplied by the compiler itself and carries no tokens.
      if (peerName === '@lingxia/view') continue;
      if (visited.has(peerName)) continue;
      const peer = findInstalled(peerName, [root, pkg.dir]);
      if (!peer) {
        if (!pkg.optionalPeers.has(peerName))
          problems.push(`${pkg.name}/package.json: UI_CSS: peer dependency ${peerName} of ${pkg.name} is not installed; install ${peerName}`);
        continue;
      }
      visited.set(peer.name, peer);
      order.push(peer);
    }
  }
  const tokens = new Map();
  for (const pkg of visited.values()) if (pkg.tokens !== undefined) tokens.set(pkg.name, pkg);
  return tokens;
}
