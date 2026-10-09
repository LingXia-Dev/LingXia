// cargo test supplies the actual Rust-generated scripts over stdin.
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { createRequire } = require('node:module');
const path = require('node:path');
const vm = require('node:vm');

const input = JSON.parse(readFileSync(0, 'utf8'));
const mode = process.argv[2];

if (mode === 'syntax') {
  const workspaceRequire = createRequire(path.resolve(__dirname, '../../../packages/package.json'));
  const { parse } = workspaceRequire('acorn');
  const scripts = [input.bootstrap, input.publication, input.stale, ...Object.values(input.pushes)];
  for (const script of scripts) {
    parse(script, { ecmaVersion: 5, sourceType: 'script' });
  }
} else if (mode === 'runtime') {
  const styles = {};
  const attributes = {};
  const events = [];
  const context = vm.createContext({
    document: {
      documentElement: {
        style: { setProperty(name, value) { styles[name] = value; } },
        setAttribute(name, value) { attributes[name] = value; },
      },
    },
  });
  // Delete the binding rather than assigning undefined: unguarded reads must throw.
  vm.runInContext(`
    this.window = this;
    delete this.globalThis;
    this.CustomEvent = function (type, options) {
      this.type = type;
      this.detail = options.detail;
    };
  `, context);
  context.dispatchEvent = (event) => {
    // The new theme and layout must already be visible when the event fires.
    assert.strictEqual(event.detail, context.lxPageChrome.layout);
    assert.equal(styles['--lx-page-chrome-top-inset'], event.detail.topInset + 'px');
    events.push(event);
  };
  assert.equal(vm.runInContext("'globalThis' in window", context), false);
  assert.throws(() => vm.runInContext('globalThis', context), { name: 'ReferenceError' });

  const run = (name) => vm.runInContext(input[name], context, { filename: name + '.js', timeout: 1000 });
  const check = (expected, theme) => {
    const layout = context.lxPageChrome.layout;
    assert.deepEqual(JSON.parse(JSON.stringify(layout)), expected);
    assert.ok(Object.isFrozen(layout));
    if (layout.capsuleRect !== null) assert.ok(Object.isFrozen(layout.capsuleRect));
    assert.equal(styles['--lx-page-chrome-top-inset'], expected.topInset + 'px');
    assert.equal(styles['--lx-page-chrome-bottom-inset'], expected.bottomInset + 'px');
    assert.equal(styles['--lx-page-chrome-capsule-inline-end-inset'], expected.capsuleInlineEndInset + 'px');
    // The capsule box the guide documents, 0px when the page has no capsule.
    for (const edge of ['top', 'right', 'bottom', 'left', 'width', 'height']) {
      const value = expected.capsuleRect ? expected.capsuleRect[edge] : 0;
      assert.equal(styles['--lx-page-chrome-capsule-' + edge], value + 'px');
    }
    assert.equal(attributes['data-theme'], theme);
    assert.equal(context.document.documentElement.style.colorScheme, theme);
    assert.equal(events.at(-1).type, 'lxpagechromechange');
    assert.strictEqual(events.at(-1).detail, layout);
  };

  run('bootstrap');
  check(input.initial, 'dark');
  assert.equal(events.length, 1);
  const api = context.lxPageChrome;
  run('publication');
  check(input.updated, 'light');
  assert.equal(events.length, 2);
  const current = api.layout;
  run('stale');
  run('bootstrap');
  assert.strictEqual(context.lxPageChrome, api);
  assert.strictEqual(api.layout, current);
  assert.equal(events.length, 2);
  check(input.updated, 'light');

  // The other pushes are no-ops before their handler exists, and call it once installed.
  const calls = [];
  for (const [name, script] of Object.entries(input.pushes)) {
    vm.runInContext(script, context, { filename: name + '.js', timeout: 1000 });
  }
  assert.equal(calls.length, 0);
  vm.runInContext(`
    window.__lingxiaApplyDisplayLanguage = function (tag, revision) { calls.push(['displayLanguage', tag, revision]); };
    window.__lingxiaApplySurfaceContext = function (ctx, revision) { calls.push(['surfaceContext', ctx.sizeClass, revision]); };
    window.__lingxiaDispatchLeaveRequest = function (reason) { calls.push(['leaveRequest', reason]); };
  `, Object.assign(context, { calls }));
  for (const [name, script] of Object.entries(input.pushes)) {
    vm.runInContext(script, context, { filename: name + '.js', timeout: 1000 });
  }
  // serde_json emits object keys sorted; the vm context has its own Array.
  assert.deepEqual(JSON.parse(JSON.stringify(calls)), [
    ['displayLanguage', 'zh-CN', 7],
    ['leaveRequest', 'back'],
    ['surfaceContext', 'regular', 3],
  ]);
} else {
  throw new Error('Unknown compatibility check: ' + mode);
}
