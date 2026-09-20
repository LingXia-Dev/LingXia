import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { spec, expect, run, reset } from '../dist/index.js';
import { createWorld, installFakeHost } from './helpers/fake-host.mjs';
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
afterEach(() => { reset(); delete globalThis.lx; delete globalThis.__LINGXIA_AUTOMATION_HOST__; });

test('empty selections fail unless explicitly allowed', async () => {
  const host = installFakeHost(createWorld(), { args: { grep: 'absent' } });
  spec('present', () => { throw new Error('must not run'); });
  await assert.rejects(run, /No tests matched/);
  host.args.passWithNoTests = '1';
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
  await assert.rejects(() => fixture.app.eval({ script: '1' }), /closed/);
});

test('retries require a reset and preserve every attempt and flaky status', async () => {
  const host = installFakeHost(createWorld(), { args: { retries: '1' } });
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
  const host = installFakeHost(createWorld(), { args: { shard: '1/2', passWithNoTests: '1' } });
  for (let i = 0; i < 10; i++) spec(`case ${i}`, { id: `id-${i}` }, () => {});
  const first = (await run()).cases.map(c => c.id);
  host.args.shard = '2/2';
  const second = (await run()).cases.map(c => c.id);
  assert.equal(new Set([...first, ...second]).size, 10);
  assert.equal(first.length + second.length, 10);
  delete host.args.shard;
  host.args.id = 'id-1';
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
    await t.app.page.testId('save').click({timeout:500, interval:10});
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
  spec('one submission', {forensics:false}, t => t.app.page.testId('save').click({interval:1}));
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
  const host = installFakeHost(createWorld(), {args:{grep:'['}});
  spec('budget', {timeout:100, timeoutCleanup:50_000}, () => {});
  await assert.rejects(run, /regular expression/i);
  delete host.args.grep;
  await run();
  assert.equal(host.events.find(e => e.type === 'case_started').watchdog_timeout_ms, 62_100);
});

test('state matchers distinguish absent, hidden, disabled and editable targets', async () => {
  const world = createWorld();
  world.add({testId:'hidden', visible:false, enabled:false, editable:false});
  world.add({testId:'input', enabled:true, editable:true});
  installFakeHost(world);
  spec('states', async t => {
    await t.expect(t.app.page.testId('absent')).toBeHidden();
    await t.expect(t.app.page.testId('hidden')).toBeAttached();
    await t.expect(t.app.page.testId('hidden')).toBeDisabled();
    await t.expect(t.app.page.testId('hidden')).not.toBeEditable();
    await t.expect(t.app.page.testId('input')).toBeEnabled();
    await t.expect(t.app.page.testId('input')).toBeEditable();
  });
  assert.equal((await run()).passed, 1);
});

test('a native pre-dispatch rejection is retried safely', async () => {
  const world = createWorld();
  const element = world.add({testId:'save'});
  const click = world.app.page.click;
  let calls = 0;
  world.app.page.click = async options => {
    if (++calls === 1) throw new Error('Element not interactable: not enabled');
    await click(options);
  };
  installFakeHost(world);
  spec('state changed before dispatch', t => t.app.page.testId('save').click({timeout:500, interval:1}));
  assert.equal((await run()).passed, 1);
  assert.equal(element.clicked, 1);
});
