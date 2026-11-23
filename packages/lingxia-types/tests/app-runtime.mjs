import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const source = readFileSync(new URL('../../../crates/lingxia-lxapp/src/appservice/scripts/App.js', import.meta.url), 'utf8');

function world() {
  const context = vm.createContext({ console, Promise, JSON });
  vm.runInContext(`
globalThis.went = [];
globalThis.lx = { nav: { go(target) { went.push(target); return Promise.resolve(); } } };
globalThis.LxAppSvc = class { constructor(app) { globalThis.app = app; } };
`, context);
  vm.runInContext(source, context);
  return (script) => vm.runInContext(script, context);
}
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const link = `{ scene: 8003, url: 'https://x.example/showcase/detail?id=7', path: 'pages/home/index', query: { id: '7' } }`;

// Cold: onOpen runs after onLaunch settles, and its target is navigated to.
{
  const run = world();
  const json = (script) => JSON.parse(run(`JSON.stringify(${script})`));
  run(`globalThis.order = [];
__registerApp({
  async onLaunch() { order.push('launch'); await Promise.resolve(); order.push('launched'); },
  onOpen(link) { order.push('open'); return { page: 'detail', params: { id: link.query.id } }; },
}, '[]');`);
  run(`app.onLaunch(${link})`);
  await settle();
  assert.deepEqual(json('order'), ['launch', 'launched', 'open']);
  assert.deepEqual(json('went'), [{ page: 'detail', params: { id: '7' } }]);
}

// Warm: onOpen runs from onShow; a show without a link and an empty return navigate nowhere.
{
  const run = world();
  const json = (script) => JSON.parse(run(`JSON.stringify(${script})`));
  run(`globalThis.shows = 0;
__registerApp({ onShow() { shows += 1; }, onOpen(link) { return link.query.id === '7' ? { tab: 'home' } : undefined; } }, '[]');`);
  run(`app.onShow({ reason: 'foreground' })`);
  run(`app.onShow({ scene: 8003, url: 'https://x.example/', query: {} })`);
  await settle();
  assert.deepEqual(json('went'), []);
  run(`app.onShow(${link})`);
  await settle();
  assert.deepEqual(json('went'), [{ tab: 'home' }]);
  assert.equal(run('shows'), 3, 'the app hook still runs');
}

// The hooks see no link plumbing: onLaunch takes nothing, onShow keeps where it lands.
{
  const run = world();
  const json = (script) => JSON.parse(run(`JSON.stringify(${script})`));
  run(`globalThis.seen = [];
__registerApp({ onLaunch(...args) { seen.push(args.length); }, onShow(args) { seen.push(args); }, onOpen() {} }, '[]');`);
  run(`app.onLaunch(${link})`);
  run(`app.onShow(${link})`);
  await settle();
  assert.deepEqual(json('seen'), [0, { path: 'pages/home/index', query: { id: '7' } }]);
}

// globalData is gone.
assert.throws(() => world()(`__registerApp({ globalData: {} }, '[]')`), /globalData is gone/);

// App.onError: serial calls after the failing stack, merged repeats, a cap, its own throw not re-reported.
{
  const realError = console.error;
  const run = world();
  const json = (script) => JSON.parse(run(`JSON.stringify(${script})`));
  run(`globalThis.now = 0; Date.now = () => now;
globalThis.got = []; globalThis.logged = [];
console.error = (...args) => logged.push(String(args[0]));
let busy = false;
__registerApp({
  async onError(failure) {
    if (busy) got.push('overlap');
    busy = true;
    got.push({ message: failure.error.message, source: failure.source, page: failure.page, repeated: failure.repeated,
      wrapped: failure.error.cause === 'plain' });
    await Promise.resolve();
    busy = false;
    if (failure.error.message === 'boom-in-handler') throw new Error('onError itself');
  },
  async onLaunch() { throw new Error('launch'); },
}, '[]');`);
  run(`app.onLaunch().catch(() => { globalThis.launchStillRejects = true; })`);
  run(`const page = { name: 'home', instance: 'p1' };
const hook = (message) => ({ error: new Error(message), source: { kind: 'hook', hook: 'every' } });
__LX_REPORT_FAILURE__(hook('flaky'), page);
__LX_REPORT_FAILURE__(hook('flaky'), page);
__LX_REPORT_FAILURE__(hook('flaky'), page);
__LX_REPORT_FAILURE__({ error: 'plain', source: { kind: 'app', hook: null } }, null);
__LX_REPORT_FAILURE__({ error: new Error('boom-in-handler'), source: { kind: 'app', hook: 'store' } }, null);
globalThis.reportedSync = got.length;`);
  assert.equal(run('reportedSync'), 0, 'never inside the failing stack');
  await settle();
  run(`now = 1500; __LX_REPORT_FAILURE__({ error: new Error('flaky'), source: { kind: 'hook', hook: 'every' } }, { name: 'home', instance: 'p1' });`);
  await settle();
  const got = json('got');
  assert.equal(run('launchStillRejects'), true);
  // In the order they happened: onLaunch's rejection settles after the reports made in this turn.
  assert.deepEqual(got.map((g) => [g.message, g.repeated]), [
    ['flaky', 0], ['plain', 0], ['boom-in-handler', 0], ['launch', 0], ['flaky', 2],
  ]);
  assert.equal(got.includes('overlap'), false, 'one call at a time');
  assert.deepEqual(got[3].source, { kind: 'app', hook: 'onLaunch' });
  assert.deepEqual(got[0].page, { name: 'home', instance: 'p1' });
  assert.equal(got[1].wrapped, true, 'a thrown non-Error is wrapped, the value in cause');
  assert.deepEqual(json('logged'), ['App.onError threw; its own failure is not reported again:']);

  // The cap: 20 calls per 10 s; the rest ride on the next call's `repeated`.
  run(`got.length = 0; now = 20000;
for (let i = 0; i < 25; i += 1) __LX_REPORT_FAILURE__({ error: new Error('e' + i), source: { kind: 'app', hook: null } }, null);`);
  await settle();
  assert.equal(json('got.length'), 20);
  run(`now = 30000; __LX_REPORT_FAILURE__({ error: new Error('after the window'), source: { kind: 'app', hook: null } }, null);`);
  await settle();
  assert.deepEqual(json('got[20]').repeated, 5);
  console.error = realError;
}

// Without onError a failure is only logged; onOpen's failure is reported as an app hook.
{
  const realError = console.error;
  const run = world();
  run(`globalThis.logged = []; console.error = (...args) => logged.push(args.map(String).join(' '));
__registerApp({ onOpen() { throw new Error('bad link'); } }, '[]');`);
  run(`app.onShow({ scene: 8003, url: 'https://x.example/', path: 'p', query: {} })`);
  await settle();
  const logged = JSON.parse(run('JSON.stringify(logged)'));
  assert.equal(logged.length, 1);
  console.error = realError;
  assert.match(logged[0], /Unhandled failure: Error: bad link/);
}
