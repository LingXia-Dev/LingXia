import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { spec, expect, run, reset } from '../dist/index.js';
import { createWorld, installFakeHost } from './helpers/fake-host.mjs';
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
afterEach(() => { reset(); delete globalThis.lx; delete globalThis.__LINGXIA_AUTOMATION_HOST__; });

test('empty selections fail unless explicitly allowed', async () => {
  const host = installFakeHost(createWorld(), { control: { grep: 'absent' } });
  spec('present', () => { throw new Error('must not run'); });
  await assert.rejects(run, /No tests matched/);
  host.control.passWithNoTests = '1';
  assert.equal((await run()).total, 0);
});

test('cleanup has a hard deadline and stops subsequent specs', async () => {
  installFakeHost(createWorld());
  let next = false;
  spec('stalled cleanup', { timeoutCleanup: 30, forensics: false }, t => {
    t.defer(() => new Promise(() => {}));
  });
  spec('cannot share contaminated state', () => { next = true; });
  const report = await run();
  assert.equal(next, false);
  assert.equal(report.partial, true);
  assert.equal(report.cases[0].status, 'failed');
  assert.equal(report.cases[0].error.phase, 'defer');
  assert.match(report.cases[0].error.message, /cleanup budget/);
  assert.equal(report.cases[1].status, 'skipped');
});

test('a timed-out body cannot regain access through cleanup', async () => {
  const world = createWorld();
  installFakeHost(world);
  let cleanup = false, next = false, fixture;
  spec('pending body', { timeout: 20, forensics: false }, t => {
    fixture = t;
    t.defer(() => { cleanup = true; });
    return new Promise(() => {});
  });
  spec('must not run', () => { next = true; });
  const report = await run();
  assert.equal(report.timeout, 1);
  assert.equal(cleanup, false);
  assert.equal(next, false);
  await assert.rejects(() => fixture.app.logic.eval(() => 1), /closed/);
});

test('retries require a reset and preserve every attempt and flaky status', async () => {
  const host = installFakeHost(createWorld(), { control: { retries: '1' } });
  let calls = 0, resets = 0, afters = 0;
  spec('intermittent', { forensics: false }, () => { expect(++calls).toBe(2); });
  await assert.rejects(run, /spec.reset/);
  spec.reset(() => { resets++; });
  spec.afterEach(() => { afters++; });
  const report = await run();
  assert.equal(resets, 2);
  assert.equal(afters, 2);
  assert.equal(report.total, 1);
  assert.equal(report.passed, 1);
  assert.equal(report.cases[0].flaky, true);
  assert.deepEqual(report.cases[0].attempts.map(a => a.status), ['failed', 'passed']);
  assert.equal(host.events.filter(e => e.type === 'case_finished').length, 2);
});

test('exact ids and shards select deterministically without overlap', async () => {
  const host = installFakeHost(createWorld(), { control: { shard: '1/2', passWithNoTests: '1' } });
  for (let i = 0; i < 10; i++) spec(`case ${i}`, { id: `id-${i}` }, () => {});
  const first = (await run()).cases.map(c => c.id);
  host.control.shard = '2/2';
  const second = (await run()).cases.map(c => c.id);
  assert.equal(new Set([...first, ...second]).size, 10);
  assert.equal(first.length + second.length, 10);
  delete host.control.shard;
  host.control.id = 'id-1';
  assert.deepEqual((await run()).cases.map(c => c.id), ['id-1']);
});

test('locator waits for enabled state, geometry stability and hit testing', async () => {
  const world = createWorld();
  const el = world.add({testId:'save', enabled:false});
  let hit = false;
  world.app.page.eval = async () => hit ? true : 'element is obscured';
  installFakeHost(world);
  spec('waits', async t => {
    const change = (async () => { await delay(30); el.enabled = true; await delay(30); hit = true; })();
    await t.app.view.testId('save').click({timeout:500, interval:10});
    await change;
  });
  assert.equal((await run()).passed, 1);
  assert.equal(el.clicked, 1);
});

test('input transport errors are not blindly retried', async () => {
  const world = createWorld();
  world.add({testId:'save'});
  let dispatched = 0;
  world.app.page.click = async () => { dispatched++; throw new Error('connection lost after dispatch'); };
  installFakeHost(world);
  spec('one submission', {forensics:false}, t => t.app.view.testId('save').click({interval:1}));
  const report = await run();
  assert.equal(dispatched, 1);
  assert.equal(report.failed, 1);
});

test('afterEach can register cleanup and cleanup failures do not skip remaining defers', async () => {
  installFakeHost(createWorld());
  const order = [];
  spec.afterEach(t => { order.push('after'); t.defer(() => { order.push('hook-defer'); throw new Error('cleanup failure'); }); });
  spec('cleanup order', {forensics:false}, t => { t.defer(() => { order.push('body-defer'); }); });
  const report = await run();
  assert.deepEqual(order, ['after', 'hook-defer', 'body-defer']);
  assert.equal(report.failed, 1);
  assert.equal(report.cases[0].error.phase, 'defer');
});

test('invalid grep is rejected and watchdog includes explicit cleanup budget', async () => {
  const host = installFakeHost(createWorld(), { control: { grep: '[' } });
  spec('budget', {timeout:100, timeoutCleanup:50_000}, () => {});
  await assert.rejects(run, /regular expression/i);
  delete host.control.grep;
  await run();
  assert.equal(host.events.find(e => e.type === 'case_started').watchdog_timeout_ms, 62_100);
});

test('state matchers distinguish absent, hidden, disabled and editable targets', async () => {
  const world = createWorld();
  world.add({testId:'hidden', visible:false, enabled:false, editable:false});
  world.add({testId:'input', enabled:true, editable:true});
  installFakeHost(world);
  spec('states', async t => {
    await t.expect(t.app.view.testId('absent')).toBeHidden();
    await t.expect(t.app.view.testId('hidden')).toBeAttached();
    await t.expect(t.app.view.testId('hidden')).toBeDisabled();
    await t.expect(t.app.view.testId('hidden')).not.toBeEditable();
    await t.expect(t.app.view.testId('input')).toBeEnabled();
    await t.expect(t.app.view.testId('input')).toBeEditable();
  });
  assert.equal((await run()).passed, 1);
});

test('a native pre-dispatch rejection is retried safely', async () => {
  const world = createWorld();
  const element = world.add({testId:'save'});
  const click = world.app.page.click;
  let calls = 0;
  world.app.page.click = async options => {
    if (++calls === 1) throw Object.assign(new Error('Element not interactable: not enabled'), { code: 'E_ELEMENT_NOT_INTERACTABLE' });
    await click(options);
  };
  installFakeHost(world);
  spec('state changed before dispatch', t => t.app.view.testId('save').click({timeout:500, interval:1}));
  assert.equal((await run()).passed, 1);
  assert.equal(element.clicked, 1);
});

test('page-scoped indexed locators preserve target on every read and input', async () => {
  const world = createWorld();
  world.add({testId:'field', visible:true, enabled:true, editable:true});
  world.add({testId:'field', visible:true, enabled:true, editable:true});
  const calls = [];
  for (const key of ['query','eval','click','fill','type']) {
    const original = world.app.page[key];
    world.app.page[key] = async options => { calls.push([key, options]); return original(options); };
  }
  world.app.page.press = async options => { calls.push(['press', options]); };
  installFakeHost(world);
  spec('scoped input', async t => {
    const input = t.app.view.testId('field', {page:'editor-instance'}).nth(1);
    await input.fill('hello');
    await input.press('Enter');
    const result = await input.query();
    expect(result.index).toBe(1);
    await t.expect(input).toHaveValue('hello');
  });
  const report = await run();
  assert.equal(report.failed,0);
  assert.ok(calls.length > 4);
  assert.ok(calls.every(([, options]) => options.page === 'editor-instance'));
  assert.equal(calls.find(([key]) => key === 'press')[1].index,1);
  assert.match(report.cases[0].steps[0].detail, /editor-instance.*\[1\]/);
});

test('hidden duplicates remain ambiguous and invalid indexes fail early', async () => {
  const world=createWorld();
  world.add({testId:'duplicate',visible:true});
  world.add({testId:'duplicate',visible:false});
  installFakeHost(world);
  spec('strict selection',{forensics:false},async t => {
    assert.throws(() => t.app.view.css('button').nth(-1), /non-negative integer/);
    await assert.rejects(t.app.view.testId('duplicate').click({timeout:20,interval:2}), /2 matches/);
  });
  assert.equal((await run()).failed,0);
});

test('host drivers are traced and retained references stop with the fixture', async () => {
  const world=createWorld();
  installFakeHost(world);
  const root=globalThis.lx.automation();
  let calls=0, retained, fixture;
  root.browser={tabs:async()=>{calls++;return [];}};
  globalThis.lx.automation=()=>root;
  spec('host trace', async t => {
    fixture=t;
    retained=t.automation.browser;
    await retained.tabs();
    await t.automation.lxapp('another').info();
  });
  const report=await run();
  assert.equal(report.failed,0);
  assert.ok(report.cases[0].steps.some(step=>step.name==='browser.tabs'));
  await assert.rejects(retained.tabs(), /closed/);
  assert.throws(()=>fixture.automation.lxapp(), /closed/);
  assert.equal(calls,1);
});

test('automation error codes and JSON data survive into reports', async () => {
  const world=createWorld();
  world.add({tag:'button'});
  world.app.page.click=async()=>{throw Object.assign(new Error('permission denied'), {code:'E_DENIED', data:{target:'page'}});};
  installFakeHost(world);
  spec('structured failure',{forensics:false},async t=>{await t.app.view.css('button').click({timeout:500});});
  const error=(await run()).cases[0].error;
  assert.equal(error.code,'E_DENIED');
  assert.deepEqual(error.data,{target:'page'});
});

test('fixture proxies keep the native receiver for branded getters', async () => {
  const world=createWorld();
  installFakeHost(world);
  class NativeRoot {
    #browser = Object.assign(function nativeDriver() {}, { tabs: async () => ['tab'] });
    get browser() { return this.#browser; }
    lxapp() { return world.app; }
  }
  const native=new NativeRoot();
  globalThis.lx.automation=()=>native;
  spec('native getter',async t=>{expect(await t.automation.browser.tabs()).toEqual(['tab']);});
  assert.equal((await run()).failed,0);
});

test('a spec that leaves the app under test closed does not fail the rest of the run', async () => {
  const world = createWorld();
  let running = true;
  const opened = [];
  const lxapps = {
    async list() { return running ? [{ appid: 'demo-app', status: 'opened' }] : []; },
    async open(options) {
      opened.push(options.appid);
      running = true;
      return { appid: options.appid, path: 'pages/home/index' };
    },
  };
  const { events } = installFakeHost(world, { control: {}, lxapps });
  spec('closes the app', async () => {
    running = false;
    throw new Error('lxapp is not active: demo-app');
  });
  spec('runs after it', async () => {});
  spec('runs later', async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ['failed', 'passed', 'passed']);
  assert.deepEqual(opened, ['demo-app'], 'reopened once, before the next spec');
  const recoveries = events.filter((event) => event.type === 'diagnostic' && event.phase === 'recovery');
  assert.equal(recoveries.length, 1);
  assert.match(recoveries[0].message,
    /\(demo-app\) was not running before "runs after it"; it stopped during or after "closes the app" \(failed\)\. Reopened it/);
  assert.equal(report.cases[1].steps[0].name, 'app.reopen');
  assert.ok(world.navCalls.some(([method]) => method === 'relaunch'), 'waits for the home page');
});

test('the run hands the app under test back running', async () => {
  const world = createWorld();
  let running = true;
  const opened = [];
  const lxapps = {
    async list() { return running ? [{ appid: 'demo-app', status: 'opened' }] : []; },
    async open(options) {
      opened.push(options.appid);
      running = true;
      return { appid: options.appid, path: 'pages/home/index' };
    },
  };
  const { events } = installFakeHost(world, { control: {}, lxapps });
  spec('passes, then the app goes away', async () => { running = false; });

  const report = await run();
  assert.equal(report.passed, 1);
  assert.deepEqual(opened, ['demo-app'], 'reopened once, after the last spec');
  const recoveries = events.filter((event) => event.type === 'diagnostic' && event.phase === 'recovery');
  assert.equal(recoveries.length, 1);
  assert.match(recoveries[0].message, /not running at the end of the run; reopened it/);
});

test('a reopen that fails is reported as recovery_failed', async () => {
  const world = createWorld();
  const lxapps = {
    async list() { return []; },
    async open() { throw new Error('bundle missing'); },
  };
  const { events } = installFakeHost(world, { control: {}, lxapps });
  spec('first', async () => {});
  spec('second', async () => {});

  await run();
  const failed = events.filter((event) => event.type === 'diagnostic' && event.phase === 'recovery_failed');
  // Before the first spec, before the second, and at the end of the run.
  assert.equal(failed.length, 3);
  assert.match(failed.at(-1).message, /at the end of the run, and reopening it failed: .*bundle missing/);
});
