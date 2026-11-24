// Shared per-lxapp View chunk and per-language catalogue emission (`lx:view/<id>`).
import { build } from 'esbuild';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { moduleBanner } from './compatibility.mjs';
import { authored, packageOf, sourceLabel } from './packages.mjs';

export const CHUNKS_MIN_RUNTIME = '0.30.0';
const MIN_TUPLE = [0, 30, 0];

function parseSemver(text) {
  if (typeof text !== 'string') return null;
  const core = text.trim().split(/[-+]/)[0];
  const parts = core.split('.').map(Number);
  if (parts.length < 3 || parts.some(n => !Number.isInteger(n) || n < 0)) return null;
  return [parts[0], parts[1], parts[2]];
}

export function meetsChunksMinRuntime(version) {
  const parsed = parseSemver(version);
  if (!parsed) return false;
  for (let i = 0; i < 3; i++) {
    if (parsed[i] !== MIN_TUPLE[i]) return parsed[i] > MIN_TUPLE[i];
  }
  return true;
}

/**
 * Decides whether `buildPages` emits a shared View chunk for `root`.
 * Gated on `minRuntime >= 0.30.0` in `lxapp.json` (or `options.minRuntime`).
 */
export async function resolveChunkPolicy(root, options = {}) {
  if (typeof options.chunks === 'boolean') return { split: options.chunks, note: null };
  let minRuntime = options.minRuntime;
  let hasManifest = false;
  if (minRuntime === undefined) {
    const manifestPath = join(root, 'lxapp.json');
    if (existsSync(manifestPath)) {
      hasManifest = true;
      try {
        const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
        minRuntime = manifest?.minRuntime;
      } catch {
        minRuntime = undefined;
      }
    }
  } else {
    hasManifest = true;
  }
  if (meetsChunksMinRuntime(minRuntime)) return { split: true, note: null };
  const note = hasManifest && minRuntime
    ? `Note: minRuntime is ${minRuntime}; raising minRuntime to ${CHUNKS_MIN_RUNTIME} enables shared View chunks and shrinks the package.`
    : null;
  return { split: false, note };
}

/**
 * Discovers package modules imported by pages that belong in the shared chunk:
 * `@liulidesign/*` modules used by any page, plus any other package module used by 2+ pages.
 */
export async function discoverSharedPackages(root, pages, {
  tsconfig,
  runtimeRoot,
  publicEntry,
  jsx,
  compiledMessages,
}) {
  const entryPoints = pages.map(page => resolve(root, page));
  if (!entryPoints.length) return [];
  const i18nDir = dirname(compiledMessages).toLowerCase();
  let scan;
  try {
    scan = await build({
      entryPoints,
      bundle: true,
      write: false,
      metafile: true,
      format: 'esm',
      target: 'es2020',
      jsxFactory: 'h',
      jsxFragment: 'Fragment',
      inject: [jsx],
      outdir: join(root, '.lingxia', '_chunk-scan'),
      absWorkingDir: root,
      logLevel: 'silent',
      tsconfigRaw: tsconfig,
      alias: { '@lingxia/view': publicEntry },
      plugins: [{
        name: 'lingxia-chunk-scan',
        setup(builder) {
          builder.onLoad({ filter: /\.css$/ }, () => ({ contents: '', loader: 'js' }));
        },
      }],
    });
  } catch {
    return [];
  }
  const counts = new Map();
  const outputs = Object.values(scan.metafile?.outputs ?? {});
  for (const out of outputs) {
    const seenInPage = new Set();
    for (const relPath of Object.keys(out.inputs ?? {})) {
      const abs = resolve(root, relPath);
      const lower = abs.toLowerCase();
      if (lower.startsWith(runtimeRoot) || lower.startsWith(i18nDir)) continue;
      if (!/\.[jt]sx?$/.test(abs)) continue;
      if (authored(root, abs)) continue;
      if (seenInPage.has(abs)) continue;
      seenInPage.add(abs);
      counts.set(abs, (counts.get(abs) ?? 0) + 1);
    }
  }
  // Select `@liulidesign/*` entry modules (any page) and other package modules used by 2+ pages.
  // Keep only top-level imported modules (or modules directly imported by pages) to avoid splitting
  // internal helper cycles into separate proxies.
  const selected = [];
  for (const [abs, count] of counts.entries()) {
    const owner = packageOf(abs);
    const isLiuli = owner?.name?.startsWith('@liulidesign/');
    if (isLiuli || count >= 2) {
      // Skip `export *` barrels so pages tree-shake only their actually imported leaf modules
      // instead of pulling an entire barrel's re-exports and class candidates.
      try {
        const src = await readFile(abs, 'utf8');
        if (/\bexport\s+\*\s+from\b/.test(src)) continue;
      } catch {
        continue;
      }
      selected.push(abs);
    }
  }
  if (!selected.length) return [];

  // Resolve exact exported names for each selected package module in one esbuild pass.
  let exportScan;
  try {
    exportScan = await build({
      entryPoints: selected,
      bundle: false,
      write: false,
      metafile: true,
      format: 'esm',
      target: 'es2020',
      jsxFactory: 'h',
      jsxFragment: 'Fragment',
      outdir: join(root, '.lingxia', '_chunk-exports'),
      absWorkingDir: root,
      logLevel: 'silent',
      tsconfigRaw: tsconfig,
    });
  } catch {
    return [];
  }
  const byEntry = new Map();
  for (const out of Object.values(exportScan.metafile?.outputs ?? {})) {
    if (!out.entryPoint) continue;
    const abs = resolve(root, out.entryPoint);
    const exports = (out.exports ?? []).filter(name => name && name !== '__esModule');
    if (exports.length) byEntry.set(abs, exports);
  }
  return selected
    .filter(abs => byEntry.has(abs))
    .sort()
    .map((abs, idx) => ({ abs, idx, exports: byEntry.get(abs) }));
}

/**
 * Emits the shared View framework chunk (`_view/<id>.js`) and per-language catalogue modules
 * (`_view/<id>/<tag>.js`) when `.lingxia/i18n/index.js` is present.
 */
export async function buildSharedChunk(root, output, {
  version,
  dev,
  literalColors,
  tsconfig,
  runtime,
  publicEntry,
  host,
  jsx,
  runtimeRoot,
  compiledMessages,
  sharedPackages,
  analyse,
  writeAtomic,
}) {
  const pkgCandidates = new Map();
  const pkgTokenPackages = new Map();
  const pkgProblems = [];

  const pkgExports = sharedPackages
    .map(({ abs, idx }) => `export * as __lxPkg_${idx} from ${JSON.stringify(abs)};`)
    .join('\n');

  const chunkEntry = [
    `export * from ${JSON.stringify(publicEntry)};`,
    `export * from ${JSON.stringify(host)};`,
    `export { configureClasses, configureFonts, configureSourceMap, enableHotSwap, registerCatalogues, registerLanguageCatalogue } from ${JSON.stringify(runtime)};`,
    `export { h, __lxFn } from ${JSON.stringify(jsx)};`,
    pkgExports,
  ].join('\n');

  const bundled = await build({
    stdin: { contents: chunkEntry, resolveDir: root },
    bundle: true,
    write: false,
    metafile: true,
    format: 'esm',
    target: 'es2020',
    minify: !dev,
    jsxFactory: 'h',
    jsxFragment: 'Fragment',
    inject: [jsx],
    logLevel: 'silent',
    tsconfigRaw: tsconfig,
    absWorkingDir: root,
    alias: { '@lingxia/view': publicEntry },
    plugins: [{
      name: 'lingxia-view-chunk',
      setup(builder) {
        builder.onLoad({ filter: /\.css$/ }, () => ({ contents: '', loader: 'js' }));
        builder.onLoad({ filter: /\.[jt]sx?$/ }, async ({ path }) => {
          if (path.toLowerCase().startsWith(runtimeRoot)) return;
          const owner = packageOf(path);
          if (owner?.tokens !== undefined) pkgTokenPackages.set(path.toLowerCase(), owner);
          const contents = await readFile(path, 'utf8');
          const loader = path.endsWith('.tsx') ? 'tsx' : path.endsWith('.ts') ? 'ts' : path.endsWith('.jsx') ? 'jsx' : 'js';
          const label = relative(root, path).split('\\').join('/');
          const analysed = analyse(path, contents, label, sourceLabel(root, path), loader, literalColors, dev);
          pkgProblems.push(...analysed.problems);
          pkgCandidates.set(path, analysed.names);
          return { contents: analysed.annotated, loader, resolveDir: dirname(path) };
        });
      },
    }],
  });

  const rawBody = bundled.outputFiles[0].text;
  const contentHash = createHash('sha256').update(rawBody).digest('hex').slice(0, 12);
  const chunkId = dev ? 'dev' : contentHash;
  // Deterministic content hash keeps untouched sibling pages valid across single-page dev swaps.
  const chunkBuildId = dev ? contentHash : undefined;
  const specifier = `lx:view/${chunkId}`;

  const viewDir = join(output, '_view');
  await mkdir(viewDir, { recursive: true });
  const banner = moduleBanner(version, dev ? { chunkBuildId } : {});
  const chunkOut = join(viewDir, `${chunkId}.js`);
  const chunkContent = `${banner}\n${rawBody}`;
  const existingChunk = existsSync(chunkOut) ? await readFile(chunkOut, 'utf8').catch(() => null) : null;
  if (existingChunk !== chunkContent) {
    await writeAtomic(chunkOut, chunkContent);
  }

  // Collect chunk inputs for `.lingxia/gpu-inputs.json` (`"lx:view"`).
  const chunkInputs = new Set();
  for (const relPath of Object.keys(bundled.metafile?.inputs ?? {})) {
    if (relPath === '<stdin>' || relPath.split(/[\\/]/).includes('node_modules')) continue;
    chunkInputs.add(relPath.split('\\').join('/'));
  }

  // Emit per-language catalogue modules `_view/<chunkId>/<tag>.js` when i18n is present.
  let catalogueChunk, catalogueSource, catalogueLanguages = [];
  if (existsSync(compiledMessages)) {
    catalogueChunk = specifier;
    const i18nDir = dirname(compiledMessages);
    const indexSrc = await readFile(compiledMessages, 'utf8');
    chunkInputs.add(relative(root, compiledMessages).split('\\').join('/'));
    const sourceMatch = indexSrc.match(/export\s+const\s+source\s*=\s*("[^"]+"|'[^']+')/);
    const sourceTag = sourceMatch ? JSON.parse(sourceMatch[1].replace(/^'|'$/g, '"')) : 'en-US';
    catalogueSource = sourceTag;
    const langImports = [...indexSrc.matchAll(/import\s+\w+\s+from\s+['"]\.\/([^'"]+)\.js['"]\s*;/g)]
      .map(m => m[1]);
    catalogueLanguages = langImports.filter(tag => existsSync(join(i18nDir, `${tag}.js`)));
    // Keep only the formatter helper (strip other languages' imports and the combined `languages` map).
    const formatterSrc = indexSrc
      .replace(/^\s*import\s+\w+\s+from\s+['"]\.\/[^'"]+['"]\s*;\s*$/gm, '')
      .replace(/^\s*export\s+const\s+languages\s*=\s*\{[^}]*\}\s*;\s*$/gm, '');
    const langOutDir = join(viewDir, chunkId);
    await mkdir(langOutDir, { recursive: true });
    for (const tag of langImports) {
      const langFile = join(i18nDir, `${tag}.js`);
      if (!existsSync(langFile)) continue;
      chunkInputs.add(relative(root, langFile).split('\\').join('/'));
      const langBundle = await build({
        stdin: {
          contents: `import __lxMessages from ${JSON.stringify(langFile)};\n${formatterSrc}\nglobalThis.__lingxiaRegisterCatalogue(${JSON.stringify(tag)}, __lxMessages, formatter, ${JSON.stringify(sourceTag)});\n`,
          resolveDir: i18nDir,
        },
        bundle: true,
        write: false,
        format: 'esm',
        target: 'es2020',
        charset: 'utf8',
        minify: !dev,
        logLevel: 'silent',
      });
      const langBanner = moduleBanner(version, dev ? { chunkBuildId } : {});
      await writeAtomic(join(langOutDir, `${tag}.js`), `${langBanner}\n${langBundle.outputFiles[0].text}`);
    }
  }

  // Reuse one JSX inject shim path so repeated builds do not accumulate temp files.
  const shimDir = join(root, '.lingxia');
  await mkdir(shimDir, { recursive: true });
  const jsxShim = join(shimDir, '_view-jsx.js');
  await writeAtomic(jsxShim, `export { h, Fragment, __lxFn } from ${JSON.stringify(specifier)};\n`);

  const sharedByPath = new Map(sharedPackages.map(entry => [entry.abs.toLowerCase(), entry]));

  return {
    chunkId,
    specifier,
    chunkBuildId,
    catalogueChunk,
    catalogueSource,
    catalogueLanguages,
    jsxShim,
    sharedByPath,
    pkgCandidates,
    pkgTokenPackages,
    pkgProblems,
    chunkInputs: [...chunkInputs],
  };
}

const VALID_ID = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

/** Generates a tiny ES module proxy that re-exports a shared package module from the View chunk,
 *  using alias bindings so reserved-word exports (e.g. `switch`) remain valid ESM. */
export function packageProxySource(specifier, entry) {
  const ns = `__lxPkg_${entry.idx}`;
  const lines = [`import { ${ns} } from ${JSON.stringify(specifier)};`];
  const named = entry.exports.filter(name => name !== 'default');
  named.forEach((name, i) => {
    const local = `__lxE_${i}`;
    const exportName = VALID_ID.test(name) ? name : JSON.stringify(name);
    lines.push(`const ${local} = ${ns}[${JSON.stringify(name)}]; export { ${local} as ${exportName} };`);
  });
  if (entry.exports.includes('default')) {
    lines.push(`export default ${ns}.default;`);
  }
  return lines.join('\n') + '\n';
}
