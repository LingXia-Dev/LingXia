// Run with Node 22.12+ after cargo build -p lingxia-cli:
// node tools/lingxia-cli/tests/view_legacy_build.mjs <lingxia> <modern-node_modules> <legacy-node_modules>
// Modern tooling: vite 8 + typescript. Legacy tooling: vite 7 + plugin-legacy 7 + terser + lightningcss + es-check.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const [cliArg, modernArg, legacyArg] = process.argv.slice(2);
assert(cliArg && modernArg && legacyArg, 'Expected CLI, modern node_modules, legacy node_modules');
const cli = path.resolve(cliArg);
const modern = path.resolve(modernArg);
const legacy = path.resolve(legacyArg);
const requireLegacy = createRequire(path.join(legacy, 'fixture.cjs'));
const { runChecks } = requireLegacy('es-check');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'lingxia-view-build-'));
const env = { ...process.env, PATH: `${path.dirname(process.execPath)}${path.delimiter}${process.env.PATH}` };

function fixture(name, tooling, config) {
  const dir = path.join(root, name);
  fs.mkdirSync(path.join(dir, 'pages/home'), { recursive: true });
  fs.symlinkSync(tooling, path.join(dir, 'node_modules'), 'dir');
  fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ name, version: '1.0.0', type: 'module', lingxia: { framework: 'html' } }));
  fs.writeFileSync(path.join(dir, 'lxapp.json'), JSON.stringify({ appId: name, appName: name, version: '1.0.0', minRuntime: '0.19.0', logic: false, pages: [{ name: 'home', path: 'pages/home/index' }] }));
  fs.writeFileSync(path.join(dir, 'pages/home/index.html'), '<!doctype html><html><head><link rel="stylesheet" href="./index.css"></head><body><div id="result"></div><script type="module" src="./entry.js"></script></body></html>');
  fs.writeFileSync(path.join(dir, 'pages/home/index.css'), '#result{color:#135723}');
  fs.writeFileSync(path.join(dir, 'pages/home/entry.js'), `window.runFixture = async function (value) { value ||= 3; const lazy = await import('./lazy.js'); document.querySelector('#result').textContent = (value?.name ?? lazy.default); }; window.runFixture();`);
  fs.writeFileSync(path.join(dir, 'pages/home/lazy.js'), 'export default "legacy-ok";');
  if (config) fs.writeFileSync(path.join(dir, 'lxapp.config.ts'), config);
  return dir;
}

function build(dir, expectedError) {
  const result = spawnSync(cli, ['build', '--release', '--skip-skill', '--progress', 'plain'], { cwd: dir, env, encoding: 'utf8', timeout: 120_000 });
  const output = result.stdout + result.stderr;
  assert.ifError(result.error);
  if (expectedError) {
    assert.notEqual(result.status, 0, output);
    assert.match(output, expectedError);
  } else {
    assert.equal(result.status, 0, output);
    const dist = path.join(dir, 'dist');
    const manifest = JSON.parse(fs.readFileSync(path.join(dist, 'lxapp.integrity.json'), 'utf8'));
    assert.deepEqual(manifest.files.map(file => file.path).sort(), walk(dist)
      .map(file => path.relative(dist, file).split(path.sep).join('/'))
      .filter(file => file !== 'lxapp.integrity.json').sort());
    for (const file of manifest.files) {
      const bytes = fs.readFileSync(path.join(dist, file.path));
      assert.equal(file.size, bytes.length, file.path);
      assert.equal(file.sha256, createHash('sha256').update(bytes).digest('hex'), file.path);
    }
  }
  console.log(`PASS ${path.basename(dir)}`);
  return path.join(dir, 'dist');
}

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(dir, entry.name);
    return entry.isDirectory() ? walk(file) : [file];
  });
}

function checkScriptReferences(dist, html) {
  for (const match of html.matchAll(/\b(?:src|data-src)="([^"\s]+)"/g)) {
    if (/^(?:[a-z][\w+.-]*:|\/\/)/i.test(match[1])) continue;
    const file = match[1].startsWith('/') ? path.join(dist, match[1]) : path.resolve(dist, 'pages/home', match[1]);
    assert(fs.existsSync(file), `Missing emitted script ${match[1]}`);
  }
}

try {
  for (const [name, config] of [
    ['modern-default', null],
    ['empty-plugins', "export default { view: { target: 'es2020', plugins: [] } }"],
  ]) {
    const dist = build(fixture(name, modern, config));
    const html = fs.readFileSync(path.join(dist, 'pages/home/index.html'), 'utf8');
    assert(!html.includes('vite-legacy-entry'));
    assert(html.includes('type="module"'));
    assert(fs.existsSync(path.join(dist, 'pages/home/view.js')));
  }
  const oldDist = build(fixture('existing-es5', modern, "export default { view: { target: 'es5' } }"));
  assert(!fs.readFileSync(path.join(oldDist, 'pages/home/index.html'), 'utf8').includes('type="module"'));
  assert(runChecks([{ ecmaVersion: 'es5', files: [path.join(oldDist, 'pages/home/view.js')] }]).success);

  const custom = fixture('custom-plugin', modern, `
    const plugins = [{
      name: 'fixture-plugin',
      transformIndexHtml: { order: 'post', handler() {
        return [{ tag: 'script', attrs: { src: '/plugin-probe.js' }, injectTo: 'head' }];
      } },
      generateBundle() {
        this.emitFile({ type: 'asset', fileName: 'plugin-probe.js', source: 'window.pluginProbe = true;' });
      },
    }];
    export default { view: { plugins } };
  `);
  fs.writeFileSync(path.join(custom, 'pages/home/index.html'), '<!doctype html><html><head></head><body>Plugin fixture</body></html>');
  const customDist = build(custom);
  const customHtml = fs.readFileSync(path.join(customDist, 'pages/home/index.html'), 'utf8');
  assert(customHtml.includes('/plugin-probe.js'));
  assert(fs.readFileSync(path.join(customDist, 'plugin-probe.js'), 'utf8').includes('pluginProbe'));
  checkScriptReferences(customDist, customHtml);

  const legacyConfig = `import legacy from '@vitejs/plugin-legacy';
    export default { view: { target: 'es5', cssTarget: 'chrome37', plugins: [
      legacy({ targets: ['chrome >= 37'], renderModernChunks: false })
    ] } };`;
  const dist = build(fixture('legacy-opt-in', legacy, legacyConfig));
  const html = fs.readFileSync(path.join(dist, 'pages/home/index.html'), 'utf8');
  assert(html.includes('vite-legacy-entry'));
  assert(!html.includes('type="module"'));
  assert(html.includes('polyfills.es5.js'));
  assert(html.indexOf('polyfills.es5.js') < html.indexOf('bridge-runtime.js'));
  const scripts = walk(dist).filter(file => file.endsWith('.js'));
  for (const [index, match] of [...html.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/gi)].entries()) {
    if (!match[1].trim()) continue;
    const file = path.join(root, `inline-${index}.js`);
    fs.writeFileSync(file, match[1]);
    scripts.push(file);
  }
  const checked = runChecks([{ ecmaVersion: 'es5', files: scripts }]);
  assert(checked.success, JSON.stringify(checked.errors));
  assert(scripts.some(file => file.includes('polyfills-legacy')));
  assert(scripts.some(file => file.includes('lazy-legacy')));
  assert(scripts.some(file => fs.readFileSync(file, 'utf8').includes('#135723')), 'Legacy CSS injection missing');
  checkScriptReferences(dist, html);

  build(fixture('missing-legacy-dependency', modern, legacyConfig), /@vitejs\/plugin-legacy/);
  for (const [name, config, error] of [
    ['invalid-plugins', 'export default { view: { plugins: {} } }', /view\.plugins/],
    ['config-variable', 'const config = { view: { target: "es5", plugins: [] } }; export default config;', /lxapp\.config|inline/],
    ['config-spread', 'const config = { view: { target: "es5", plugins: [] } }; export default { ...config };', /spread|direct/],
    ['view-variable', 'const view = { target: "es5", plugins: [] }; export default { view };', /view.*inline/],
    ['dynamic-target', 'const target = "es5"; export default { view: { target, plugins: [] } };', /view\.target/],
    ['mutated-target', 'function change(config) { config.view.target = "es2020"; return config; } export default change({ view: { target: "es5", plugins: [] } });', /view\.target/],
  ]) build(fixture(name, modern, config), error);
  build(fixture('broken-config', modern, 'import missing from "missing-lxapp-config-dependency"; export default { alias: missing }'), /missing-lxapp-config-dependency/);
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
