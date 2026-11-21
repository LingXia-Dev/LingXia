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
