// Shared per-lxapp View chunk and per-language catalogue emission (`lx:view/<id>`).
import { build } from 'esbuild';
import ts from 'typescript';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { moduleBanner } from './compatibility.mjs';
import { authored, packageOf, sourceLabel } from './packages.mjs';
import { scriptModule, scriptLoader } from './source.mjs';

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

/** A module of re-exports and types only: a copy per page holds no state of its own. */
function pureReexports(path, contents) {
  const file = ts.createSourceFile(path, contents, ts.ScriptTarget.ES2020, false);
  return file.statements.every(node =>
    (ts.isExportDeclaration(node) && (node.moduleSpecifier || node.isTypeOnly))
    || (ts.isImportDeclaration(node) && node.importClause?.isTypeOnly)
    || ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node));
}

// esbuild's load paths and metafile paths can differ in drive-letter case on Windows.
const pathKey = path => process.platform === 'win32' ? path.toLowerCase() : path;

/** Each metafile input's imports, keyed and listed by `pathKey` of the absolute path. */
function importGraph(root, metafile) {
  const graph = new Map();
  for (const [rel, input] of Object.entries(metafile?.inputs ?? {})) {
    graph.set(pathKey(resolve(root, rel)), (input.imports ?? []).filter(i => !i.external).map(i => pathKey(resolve(root, i.path))));
  }
  return graph;
}

/**
 * Discovers package modules imported by pages that belong in the shared chunk:
 * `@liulidesign/*` modules used by any page, any other package module used by 2+ pages, and every
 * package module those import, so no stateful module is bundled both in the chunk and in a page.
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
  // `@lingxia/view` resolves to the compiler's own runtime, which is never a shared package module.
  const viewEntry = {
    name: 'lingxia-chunk-scan',
    setup(builder) {
      builder.onResolve({ filter: /^@lingxia\/view$/ }, () => ({ path: publicEntry }));
      builder.onLoad({ filter: /\.css$/ }, () => ({ contents: '', loader: 'js' }));
    },
  };
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
      plugins: [viewEntry],
    });
  } catch {
    // The page build reports what does not compile; meanwhile nothing is shared.
    return [];
  }
  // CommonJS stays per page: a proxy cannot give its named imports esbuild's interop.
  const commonjs = new Set(Object.entries(scan.metafile?.inputs ?? {})
    .filter(([, input]) => input.format === 'cjs').map(([rel]) => resolve(root, rel)));
  const eligible = abs => {
    const lower = abs.toLowerCase();
    return scriptModule.test(abs) && !commonjs.has(abs) && !lower.startsWith(runtimeRoot)
      && !lower.startsWith(i18nDir) && !authored(root, abs);
  };
  const counts = new Map();
  for (const out of Object.values(scan.metafile?.outputs ?? {})) {
    for (const rel of new Set(Object.keys(out.inputs ?? {}))) {
      const abs = resolve(root, rel);
      if (eligible(abs)) counts.set(abs, (counts.get(abs) ?? 0) + 1);
    }
  }
  const barrels = new Map();
  const barrel = async abs => {
    if (!barrels.has(abs)) {
      let pure = true;
      try { pure = pureReexports(abs, await readFile(abs, 'utf8')); } catch {}
      barrels.set(abs, pure);
    }
    return barrels.get(abs);
  };
  // Pure barrels stay per page, so a page proxies and styles only the leaves it imports.
  const graph = importGraph(root, scan.metafile);
  const realPath = new Map(Object.keys(scan.metafile?.inputs ?? {}).map(rel => [pathKey(resolve(root, rel)), resolve(root, rel)]));
  const selected = new Set();
  const stack = [];
  for (const [abs, count] of counts) {
    if (packageOf(abs)?.name?.startsWith('@liulidesign/') || count >= 2) stack.push(abs);
  }
  const visited = new Set();
  while (stack.length) {
    const abs = stack.pop();
    if (visited.has(abs) || !eligible(abs)) continue;
    visited.add(abs);
    // Only what some page bundles can be bundled twice; the chunk build pulls in the rest itself.
    if (counts.has(abs) && !(await barrel(abs))) selected.add(abs);
    for (const next of graph.get(pathKey(abs)) ?? []) stack.push(realPath.get(next) ?? next);
  }
  if (!selected.size) return [];

  // Bundled, so `export *` inside a selected module lists the names it re-exports.
  let exportScan;
  try {
    exportScan = await build({
      entryPoints: [...selected],
      bundle: true,
      splitting: true,
      write: false,
      metafile: true,
      format: 'esm',
      target: 'es2020',
      jsxFactory: 'h',
      jsxFragment: 'Fragment',
      inject: [jsx],
      outdir: join(root, '.lingxia', '_chunk-exports'),
      absWorkingDir: root,
      logLevel: 'silent',
      tsconfigRaw: tsconfig,
      plugins: [viewEntry],
    });
  } catch {
    return [];
  }
  const byEntry = new Map();
  for (const out of Object.values(exportScan.metafile?.outputs ?? {})) {
    if (!out.entryPoint) continue;
    const exports = (out.exports ?? []).filter(name => name && name !== '__esModule');
    if (exports.length) byEntry.set(resolve(root, out.entryPoint), exports);
  }
  return [...selected]
    .filter(abs => byEntry.has(abs))
    .sort()
    .map((abs, idx) => ({ abs, idx, exports: byEntry.get(abs) }));
}

/**
 * Emits the shared View framework chunk (`_view/<id>.js`) and per-language catalogue modules
 * (`_view/<id>/<tag>.js`) when the compiled i18n module is present. `closure(abs)` lists what a
 * page proxying shared module `abs` takes on: each chunk module it reaches, with the classes,
 * source file, problems and package that bundling the module into the page would have added.
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
  shimDir,
}) {
  const modules = new Map();

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
        builder.onLoad({ filter: /\.json$/ }, async ({ path }) => {
          const names = new Set();
          const walk = value => {
            if (typeof value === 'string') { for (const name of value.split(/\s+/)) if (name) names.add(name); }
            else if (value && typeof value === 'object') Object.values(value).forEach(walk);
          };
          try { walk(JSON.parse(await readFile(path, 'utf8'))); } catch {}
          modules.set(pathKey(path), { names, file: null, problems: [], owner: null });
          return undefined;
        });
        builder.onLoad({ filter: scriptModule }, async ({ path }) => {
          if (path.toLowerCase().startsWith(runtimeRoot)) return;
          const contents = await readFile(path, 'utf8');
          const loader = scriptLoader(path);
          const label = relative(root, path).split('\\').join('/');
          const analysed = analyse(path, contents, label, sourceLabel(root, path), loader, literalColors, dev);
          const owner = authored(root, path) ? null : packageOf(path);
          modules.set(pathKey(path), { names: analysed.names, file: analysed.file, problems: analysed.problems, owner });
          return { contents: analysed.annotated, loader, resolveDir: dirname(path) };
        });
      },
    }],
  });

  const graph = importGraph(root, bundled.metafile);
  const closures = new Map();
  const closure = abs => {
    abs = pathKey(abs);
    if (closures.has(abs)) return closures.get(abs);
    const reached = [], seen = new Set(), stack = [abs];
    while (stack.length) {
      const path = stack.pop();
      if (seen.has(path)) continue;
      seen.add(path);
      const record = modules.get(path);
      if (record) reached.push({ path, ...record });
      stack.push(...(graph.get(path) ?? []));
    }
    closures.set(abs, reached);
    return reached;
  };

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
  // A dev chunk that moved invalidates every page built against the one it replaces.
  const replacedBuildId = dev && existingChunk && existingChunk !== chunkContent
    ? (/"chunkBuildId":"([^"]+)"/.exec(existingChunk.split('\n', 1)[0])?.[1] ?? null)
    : null;
  if (existingChunk !== chunkContent) {
    await writeAtomic(chunkOut, chunkContent);
  }

  // Collect chunk inputs for the dev inputs record (`"lx:view"`).
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
    if (!sourceMatch) throw Error(`${relative(root, compiledMessages)}: no \`export const source\`; rebuild the lxapp's i18n`);
    const sourceTag = JSON.parse(sourceMatch[1].replace(/^'|'$/g, '"'));
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
    for (const tag of catalogueLanguages) {
      const langFile = join(i18nDir, `${tag}.js`);
      chunkInputs.add(relative(root, langFile).split('\\').join('/'));
      // The declared order goes along, so a language resolves as it does with every catalogue loaded.
      const langBundle = await build({
        stdin: {
          contents: `import __lxMessages from ${JSON.stringify(langFile)};\n${formatterSrc}\nglobalThis.__lingxiaRegisterCatalogue(${JSON.stringify(tag)}, __lxMessages, formatter, ${JSON.stringify(sourceTag)}, ${JSON.stringify(catalogueLanguages)});\n`,
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

  // One shim per mode beside the inputs record, so a dev session and a build never share one.
  await mkdir(shimDir, { recursive: true });
  const jsxShim = join(shimDir, dev ? '_view-jsx-dev.js' : '_view-jsx-build.js');
  await writeAtomic(jsxShim, `export { h, Fragment, __lxFn } from ${JSON.stringify(specifier)};\n`);

  const sharedByPath = new Map(sharedPackages.map(entry => [entry.abs.toLowerCase(), entry]));

  return {
    chunkId,
    specifier,
    chunkBuildId,
    replacedBuildId,
    catalogueChunk,
    catalogueSource,
    catalogueLanguages,
    jsxShim,
    sharedByPath,
    closure,
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
