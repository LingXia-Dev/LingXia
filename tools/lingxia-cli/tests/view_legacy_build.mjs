// Run with Node 22.12+ after cargo build -p lingxia-cli:
// node tools/lingxia-cli/tests/view_legacy_build.mjs <lingxia> [<modern-node_modules> <legacy-node_modules>]
// With no dependency paths, install isolated, pinned tooling for CI.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const [cliArg, modernArg, legacyArg] = process.argv.slice(2);
assert(cliArg && Boolean(modernArg) === Boolean(legacyArg), 'Expected CLI and optionally both dependency directories');
const cli = path.resolve(cliArg);
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'lingxia-view-build-'));
const env = { ...process.env, PATH: `${path.dirname(process.execPath)}${path.delimiter}${process.env.PATH}` };
function tooling(name, supplied, dependencies) {
  if (supplied) return path.resolve(supplied);
  const dir = path.join(root, name);
  fs.mkdirSync(dir);
  fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({ private: true, dependencies }));
  const result = spawnSync(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['install', '--no-audit', '--no-fund'], {
    cwd: dir, env, encoding: 'utf8', timeout: 240_000,
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  return path.join(dir, 'node_modules');
}
const modern = tooling('modern-tooling', modernArg, { vite: '8.3.0', typescript: '5.9.3' });
const legacy = tooling('legacy-tooling', legacyArg, {
  vite: '7.3.6', '@vitejs/plugin-legacy': '7.2.1', typescript: '5.9.3',
  terser: '5.51.2', lightningcss: '1.33.0', 'es-check': '9.6.1',
});
const requireLegacy = createRequire(path.join(legacy, 'fixture.cjs'));
const { runChecks } = requireLegacy('es-check');

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
    assert(!fs.existsSync(path.join(dist, 'pages/pages_home_index/index.html')), 'Intermediate HTML must not ship');
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
    ['config-variable-no-plugins', 'const config = {}; export default config;'],
    ['config-spread-no-plugins', 'const defaults = {}; export default { ...defaults };'],
    ['view-variable-no-plugins', 'const view = { target: "es2020" }; export default { view };'],
    ['dynamic-target-no-plugins', 'const target = process.env.FIXTURE_TARGET || "es2020"; export default { view: { target } };'],
    ['config-variable', 'const config = { view: { target: "es2020", plugins: [] } }; export default config;'],
    ['config-spread', 'const config = { view: { target: "es2020", plugins: [] } }; export default { ...config };'],
    ['view-variable', 'const view = { target: "es2020", plugins: [] }; export default { view };'],
    ['constant-target', 'const target = "es2020"; export default { view: { target, plugins: [] } };'],
    ['empty-plugins', "export default { view: { target: 'es2020', plugins: [] } }"],
  ]) {
    const dist = build(fixture(name, modern, config));
    const html = fs.readFileSync(path.join(dist, 'pages/home/index.html'), 'utf8');
    assert(!html.includes('vite-legacy-entry'));
    assert(html.includes('type="module"'));
    assert(fs.existsSync(path.join(dist, 'pages/home/view.js')));
  }
  for (const [name, config] of [
    ['existing-es5', "export default { view: { target: 'es5' } }"],
    ['existing-es5-variable', "const config = { view: { target: 'es5' } }; export default config;"],
    ['existing-es5-spread', "const defaults = { view: { target: 'es5' } }; export default { ...defaults };"],
  ]) {
    const oldDist = build(fixture(name, modern, config));
    assert(!fs.readFileSync(path.join(oldDist, 'pages/home/index.html'), 'utf8').includes('type="module"'));
    assert(runChecks([{ ecmaVersion: 'es5', files: [path.join(oldDist, 'pages/home/view.js')] }]).success);
    assert(!fs.existsSync(path.join(oldDist, 'pages/pages_home_index')), 'Existing ES5 entry must not be duplicated');
  }

  const plain = fixture('static-without-tooling', modern, 'const config = {}; export default { ...config };');
  fs.unlinkSync(path.join(plain, 'node_modules'));
  fs.writeFileSync(path.join(plain, 'pages/home/index.html'), '<!doctype html><html><head></head><body>Static page</body></html>');
  build(plain);

  const custom = fixture('custom-plugin', modern, `
    const plugins = [{
      name: 'fixture-plugin',
      transformIndexHtml: { order: 'post', handler() {
        return [{ tag: 'script', attrs: { src: '/plugin-probe.js' }, injectTo: 'head' }];
      } },
      generateBundle() {
        this.emitFile({ type: 'asset', fileName: 'plugin-probe.js', source: 'window.pluginProbe = true;' });
        this.emitFile({ type: 'asset', fileName: 'pages/plugin-data.json', source: '{}' });
      },
    }];
    const view = { plugins };
    const config = { view };
    export default { ...config };
  `);
  fs.writeFileSync(path.join(custom, 'pages/home/index.html'), '<!doctype html><html><head></head><body>Plugin fixture</body></html>');
  const customDist = build(custom);
  const customHtml = fs.readFileSync(path.join(customDist, 'pages/home/index.html'), 'utf8');
  assert(customHtml.includes('/plugin-probe.js'));
  assert(fs.readFileSync(path.join(customDist, 'plugin-probe.js'), 'utf8').includes('pluginProbe'));
  assert(fs.existsSync(path.join(customDist, 'pages/plugin-data.json')));
  checkScriptReferences(customDist, customHtml);

  const legacyConfig = `import legacy from '@vitejs/plugin-legacy';
    export default { view: { target: 'es5', cssTarget: 'chrome37', plugins: [
      legacy({ targets: ['chrome >= 37'], renderModernChunks: false })
    ] } };`;
  for (const [name, config, dual] of [
    ['legacy-opt-in', legacyConfig, false],
    ['legacy-without-target', legacyConfig.replace("target: 'es5',", ''), false],
    ['legacy-other-target', legacyConfig.replace("target: 'es5'", "target: 'es2020'"), false],
    ['legacy-dual-output', legacyConfig.replace("target: 'es5',", '').replace('renderModernChunks: false', 'renderModernChunks: true'), true],
  ]) {
    const dir = fixture(name, legacy, config);
    const dist = build(dir);
    const html = fs.readFileSync(path.join(dist, 'pages/home/index.html'), 'utf8');
    assert(html.includes('vite-legacy-entry'));
    assert.equal(html.includes('type="module"'), dual);
    assert(html.includes('polyfills.es5.js'));
    assert(html.indexOf('polyfills.es5.js') < html.indexOf('bridge-runtime.js'));
    const scripts = walk(dist).filter(file => file.endsWith('.js') && (!dual || file.includes('-legacy')));
    for (const file of scripts) {
      const raw = path.join(dir, '.lingxia/view-build/html/dist', path.relative(dist, file));
      assert.equal(fs.readFileSync(file, 'utf8'), fs.readFileSync(raw, 'utf8'), 'Release must preserve plugin output');
    }
    for (const [index, match] of [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)].entries()) {
      if (!match[2].trim() || /type="module"/.test(match[1])) continue;
      const file = path.join(root, `inline-${index}.js`);
      fs.writeFileSync(file, match[2]);
      scripts.push(file);
    }
    const checked = runChecks([{ ecmaVersion: 'es5', files: scripts }]);
    assert(checked.success, JSON.stringify(checked.errors));
    assert(scripts.some(file => file.includes('polyfills-legacy')));
    assert(scripts.some(file => file.includes('lazy-legacy')));
    assert(scripts.some(file => fs.readFileSync(file, 'utf8').includes('#135723')), 'Legacy CSS injection missing');
    checkScriptReferences(dist, html);
  }

  build(fixture('missing-legacy-dependency', modern, legacyConfig), /@vitejs\/plugin-legacy/);
  for (const [name, config, error] of [
    ['invalid-plugins', 'export default { view: { plugins: {} } }', /view\.plugins/],
    ['dynamic-plugin-target', 'const target = process.env.FIXTURE_TARGET || "es5"; export default { view: { target, plugins: [] } };', /statically resolvable/],
    ['mutated-target', 'function change(config) { config.view.target = "es2020"; return config; } export default change({ view: { target: "es5", plugins: [] } });', /view\.target/],
  ]) build(fixture(name, modern, config), error);
  build(fixture('broken-config', modern, 'import missing from "missing-lxapp-config-dependency"; export default { alias: missing }'), /missing-lxapp-config-dependency/);
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
