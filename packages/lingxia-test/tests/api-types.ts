import {
  spec, expect, rawAutomation, TEST_ERROR_CODES, TimeoutError, type AnyLogicPage, type Fixture, type AutomationErrorCode, type ClockAdvance, type ClockState, type LogicPage, type NetworkCall, type ProfileCheckpoint, type TestApp, type TestErrorCode,
} from '../dist/index.js';
import type { FailureRecord, JsonReport, TagSummary } from '@lingxia/test/report';
import { run, trackPublicSurface, VERSION } from '@lingxia/test/runner';
import { AUTOMATION_ERROR_CODES, type Automation, type LxAppDriver, type PageDriver, type PageQueryResult } from '@lingxia/types/automation';

spec('typed test boundary', async t => {
  const app: TestApp = t.automation.lxapp('example');
  const input = app.view.testId('input', {page:'editor'}).nth(0);
  await input.press('Enter');
  const element: PageQueryResult = await input.query();
  if (element.exists) element.rect.width.toFixed();
  const state = await app.logic.eval(() => ({ ready: true }));
  state.ready.valueOf();
  // @ts-expect-error The fixture has no string eval; a script string is for the raw driver.
  await app.eval({script:'1'});
  await t.automation.browser.tabs();
  const landed = await app.nav.to({page:'editor', waitUntil:'commit'});
  landed.webviewAttached.valueOf();
  landed.instanceId?.toUpperCase();
  await t.reject(() => app.view.css('#save', {page:'devices'}).click(), {code:'E_PAGE_NOT_ACTIVE'});
  await app.nav.back({waitUntil:'ready', timeout:5_000});
  await app.nav.relaunch({page:'editor', timeout:5_000});
  // @ts-expect-error Fixture waits take `timeout`; `timeoutMs` is the raw driver's.
  await app.nav.to({page:'editor', timeoutMs:5_000});
  const stack: string[] = (await app.nav.stack()).map((page) => page.path);
  void stack;
  // @ts-expect-error The nav option is `waitUntil`; `waitFor` is the page/locator method.
  await app.nav.to({page:'editor', waitFor:'ready'});
  await input.waitFor({state:'attached'});
  await input.waitFor({state:'inViewport'});
  const rows = app.view.css('li').filter({hasText:/ready/i});
  await rows.first().click({force:true, timeout:1_000});
  await rows.last().fill('x', {force:true});
  await expect(rows.nth(1)).toBeInViewport();
  await expect(rows.first()).toContainText('ready');
  await expect(rows.first()).toHaveAttribute('aria-selected', 'true');
  await expect(rows.first()).not.toHaveAttribute('disabled');
  await app.view.css('#save').click({force:true});
  // @ts-expect-error There is no `t.app.page`: the view holds the locators.
  app.page.testId('input');
  // @ts-expect-error Actions do not poll on an interval; assertions do.
  await input.click({interval:10});
  // @ts-expect-error A fixture route is removed with `remove()`.
  await (await app.network.route('**/x', {json:{}})).unroute();
  // @ts-expect-error `type` has no forced mode.
  await input.type('x', {force:true});
  // @ts-expect-error filter needs hasText.
  app.view.css('li').filter({});
  if (!state.ready) t.skip('not ready');
  // @ts-expect-error Test context has no DOM.
  document.querySelector('button');
  // @ts-expect-error Queries read once; they do not accept ignored retry options.
  input.query({timeout:100});
  // @ts-expect-error Locators require an explicit page string.
  app.view.css('button', {page:123});
});

declare const raw: Automation;
// @ts-expect-error Raw drivers have single-dispatch APIs, not test locators.
raw.lxapp().page.testId('input');

raw.browser.wait({visible:'#ready'});
// @ts-expect-error Empty waits cannot express a condition.
raw.browser.wait({timeoutMs:100});
// @ts-expect-error Choose one condition, not two competing waits.
raw.browser.wait({loaded:true, visible:'#ready'});
// @ts-expect-error false is not a wait condition.
raw.browser.wait({loaded:false});
async function navigationResult() {
  const result = await raw.browser.eval<number>({js:'1', waitNavigation:true});
  result.value.toFixed();
  result.navigation.elapsed_ms.toFixed();
}
void navigationResult;

async function browserElement() {
  const element = await raw.browser.query({css:'button'});
  element.rect?.width.toFixed();
  // @ts-expect-error Browser query does not return the lxapp match index.
  element.index;
  // @ts-expect-error Browser text may be absent even when exists is true.
  const text: string = element.text;
  void text;
}
void browserElement;

spec.fail('known quota failure', { expected: { message: /quota/ } }, async () => {});
// @ts-expect-error `expected` belongs to spec.fail only.
spec('plain spec', { expected: { code: 'E_TIMEOUT' } }, async () => {});
// @ts-expect-error An app code is accepted only once the app declares it in `AppErrorCodes`.
spec.fail('app code', { expected: { code: 'E_QUOTA' } }, async () => {});
// @ts-expect-error A removed code does not compile: a driver timeout is `E_TIMEOUT`.
spec.fail('removed code', { expected: { code: 'E_EVAL_TIMEOUT' } }, async () => {});

spec('typed Logic access', async t => {
  // Function-form Logic eval: scope is typed, args are JSON, result is inferred.
  const count: number = await t.app.logic.eval(({ getCurrentPages }) => getCurrentPages().length);
  count.toFixed();
  const route: string = await t.app.logic.eval(async ({ getCurrentPages }, index: number) => {
    const pages = getCurrentPages();
    return pages[pages.length - 1 - index].route;
  }, 0);
  route.toUpperCase();
  const envPath = await t.app.logic.eval(({ lx }) => lx.env.USER_DATA_PATH);
  envPath.valueOf();
  // @ts-expect-error Logic API members are checked against the published `lx`.
  await t.app.logic.eval(({ lx }) => lx.noSuchApi());
  // @ts-expect-error Arguments must be JSON values; functions do not cross the boundary.
  await t.app.logic.eval((_scope, callback: () => void) => callback(), () => {});
  // @ts-expect-error Arguments are checked against the function's parameters.
  await t.app.logic.eval((_scope, id: string) => id, 42);
  // @ts-expect-error `undefined` is not JSON; it would arrive as `null` or not at all.
  await t.app.logic.eval((_scope, ids: (string | undefined)[]) => ids, ['a', undefined]);
  // @ts-expect-error The fixture takes functions; a script string is for the raw driver.
  await t.app.logic.eval({ script: 'return true' });
  // @ts-expect-error Logic eval lives on `t.app.logic`.
  await t.app.eval(() => 1);

  // View (WebView) eval: without the DOM lib, `document` is a minimal ViewDocument.
  const title: string = await t.app.view.eval(({ document }) => document.title);
  title.toUpperCase();
  const label = await t.app.view.eval(({ document }, id: string) => document.querySelector(`#${id}`)?.textContent ?? null, 'save');
  label?.toUpperCase();
  const value = await t.app.view.eval(({ document }) => document.querySelector('input')?.value);
  value?.toUpperCase();
  const disabled = await t.app.view.eval(({ document }) => document.querySelector('button')?.getAttribute('aria-disabled'));
  disabled?.toUpperCase();
  const surfaceTitle: string = await t.app.view.eval({ page: 'surface' }, ({ document }, suffix: string) => document.title + suffix, '!');
  surfaceTitle.toUpperCase();
  const slow: number = await t.app.logic.eval({ timeout: 30_000 }, ({ lx }, n: number) => n + lx.env.USER_DATA_PATH.length, 1);
  slow.toFixed();
  await t.app.view.eval({ page: 'surface', timeout: 20_000 }, ({ document }) => document.title);
  // @ts-expect-error A page target is `{ page }`.
  await t.app.view.eval({ css: '#x' }, ({ document }) => document.title);
  // @ts-expect-error ViewDocument is minimal: no DOM writes without the DOM lib.
  await t.app.view.eval(({ document }) => document.write('x'));
  // @ts-expect-error The view takes functions; a script string is for the raw driver.
  await t.app.view.eval({ script: '1 + 1' });
  // @ts-expect-error The view exposes locators, not raw element methods.
  await t.app.view.click({ css: '#save' });
  // @ts-expect-error The view exposes locators, not raw element methods.
  await t.app.view.waitFor({ css: '#save' });
  await t.app.view.screenshot();
  const cart = t.app.view.page('cart');
  await cart.testId('total').click();
  const cartTitle: string = await cart.eval(({ document }) => document.title);
  cartTitle.toUpperCase();
  await cart.page('checkout').screenshot();

  // Results are typed as JSON carries them.
  const shaped = await t.app.logic.eval(() => ({ id: 'a', at: 1, tags: ['x'] as const, save() {} }));
  shaped.id.toUpperCase(); shaped.tags[0].toUpperCase();
  // @ts-expect-error A method does not cross the boundary.
  shaped.save();
  const when = await t.app.logic.eval(() => new Date());
  // @ts-expect-error A Date arrives as a string, so it is typed `never`.
  when.getTime();
  const body = await t.app.view.eval(({ document }) => document.body);
  // @ts-expect-error A DOM element does not cross the boundary.
  body.tagName;
  const anyValue = await t.app.logic.eval(() => JSON.parse('1') as any);
  anyValue.whatever;
  const maybe = await t.app.view.eval(({ document }) => document.querySelector('x')?.textContent ?? undefined);
  maybe?.toUpperCase();
  interface Devices { devices: { id: string }[] }
  const data = await t.app.logic.data<Devices>({ page: 'devices' });
  data.devices[0].id.toUpperCase();
  // Untyped pages take any method name; the result is unknown.
  const renamed = await t.app.logic.call('rename', 'dev-1', { name: 'Office' });
  void renamed;
  // @ts-expect-error call arguments must be JSON values.
  await t.app.logic.call('rename', undefined);
  // A page type restricts the method name and types the result.
  interface DevicesPage extends LogicPage<Devices> {
    rename(id: string, patch: { name: string }): Promise<boolean>;
    count(): number;
  }
  const done: boolean = await t.app.logic.call<DevicesPage, 'rename'>('rename', 'dev-1', { name: 'Office' });
  done.valueOf();
  const either = await t.app.logic.call<DevicesPage>('count');
  void either;
  const slowDone: boolean = await t.app.logic.call<DevicesPage, 'rename'>({ timeout: 30_000 }, 'rename', 'dev-1', { name: 'Office' });
  slowDone.valueOf();
  const fired: void = await t.app.logic.call<DevicesPage>({ wait: false }, 'rename', 'dev-1', { name: 'Office' });
  void fired;
  // @ts-expect-error `wait` is a boolean.
  await t.app.logic.call({ wait: 'no' }, 'rename');
  // @ts-expect-error The method must be one the page type declares.
  await t.app.logic.call<DevicesPage>('remove');
  // @ts-expect-error `AnyLogicPage` is the untyped default; it still has to be a page.
  await t.app.logic.call<{ route: number }>('x');
  const untyped: AnyLogicPage = null as unknown as AnyLogicPage;
  void untyped.anything;

  // waitFor resolves to the accepted value; the test is `until`.
  const loaded: Devices = await t.waitFor(() => t.app.logic.data<Devices>(), { until: (d) => d.devices.length > 0, timeout: 2_000 });
  loaded.devices.length.toFixed();
  const truthy: string = await t.waitFor(async () => 'ok');
  truthy.toUpperCase();
  // @ts-expect-error until receives the read value.
  await t.waitFor(() => 1, { until: (value: string) => value.length > 0 });
  // @ts-expect-error No positional callback: pass { until }.
  await t.waitFor(() => 1, (value: number) => value > 0);

  // One expect: a locator retries, a value checks once, expect.poll(read) retries.
  await expect(t.app.view.testId('save')).toBeVisible();
  await expect.poll(() => t.app.logic.data<Devices>(), { timeout: 2_000 }).toEqual({ devices: [] });
  await expect.poll(async () => [1, 2]).toHaveLength(2);
  await expect.poll(async () => 3).toBeGreaterThan(2);
  // Dialogs: toasts are observed, modals and action sheets answered.
  await expect.poll(() => t.app.dialogs.toasts()).toContainEqual(expect.objectContaining({ title: 'Saved' }));
  await t.app.dialogs.answerNextModal({ confirm: true });
  await t.app.dialogs.answerNextActionSheet({ index: 0 });
  await t.app.dialogs.answerNextActionSheet({ cancel: true });
  const [modal] = await t.app.dialogs.modals();
  const confirmed: boolean | undefined = modal?.answer?.confirm;
  const [sheet] = await t.app.dialogs.actionSheets();
  const items: string[] | undefined = sheet?.items;
  // @ts-expect-error A modal answer is { confirm }.
  await t.app.dialogs.answerNextModal(true);
  // @ts-expect-error An action sheet answer is { index } or { cancel: true }.
  await t.app.dialogs.answerNextActionSheet({ cancel: false });
  expect([{ a: 1 }]).toContainEqual(expect.objectContaining({ a: 1 }));
  expect(3).toBe(3);
  expect([1, 2]).toHaveLength(2);
  expect({ id: 'd1' }).toMatchSchema('Device');
  expect(() => { throw new Error('x'); }).toThrow('x');
  const loose: any = { length: 1 };
  expect(loose).toHaveLength(1);
  // @ts-expect-error A once-check is synchronous: it returns no promise to await on.
  expect(3).toBe(3).then;
  // @ts-expect-error Locator matchers are not value matchers.
  await expect(t.app.view.testId('save')).toBe(1);
  // @ts-expect-error Value matchers are not locator matchers.
  expect(3).toBeVisible();
  // @ts-expect-error A promise is neither a value nor a read: await it, or poll it.
  expect(t.app.logic.data()).toEqual({});
  const readCount = () => 1;
  expect(readCount).not.toThrow();
  // @ts-expect-error A function is only called by toThrow; a read to retry is expect.poll(read).
  expect(readCount).toBe(1);
  // @ts-expect-error Nor does a function subject take other value matchers.
  expect(readCount).toBeTruthy();
  // @ts-expect-error `await expect(value)` without a matcher checks nothing.
  await expect(3);
  // @ts-expect-error `await expect(locator)` without a matcher checks nothing.
  await expect(t.app.view.testId('save'));
  // @ts-expect-error `await expect.poll(read)` without a matcher checks nothing.
  await expect.poll(() => 1);
  // @ts-expect-error Nor does `.not` alone.
  await expect(3).not;
  // @ts-expect-error A removed code does not compile in `t.reject` either.
  await t.reject(() => t.app.logic.eval(() => 1), { code: 'E_EVAL_TIMEOUT' });

  // Args may be missing; t.arg narrows or throws.
  const baseUrl: string = t.arg('baseUrl');
  const mode: string = t.arg('mode', { default: 'mock' });
  const optional = t.arg('token', { required: false });
  // @ts-expect-error An optional arg may be undefined.
  optional.toUpperCase();
  void baseUrl; void mode;
});

// @ts-expect-error The fixture has no expect of its own: import `expect`.
type FixtureHasNoExpect = Fixture['expect'];
// @ts-expect-error One reader per input: `t.arg(name, { required: false })`.
type FixtureHasNoArgRecord = Fixture['args'];
// @ts-expect-error Profiles are per app: `t.app.profile`.
type FixtureHasNoProfile = Fixture['profile'];
// @ts-expect-error Another lxapp is `t.automation.lxapp(id)`.
type FixtureHasNoApps = Fixture['apps'];

// One call record across routes, network and scenarios; removers resolve void.
spec('network calls', async (t) => {
  const route = await t.app.network.route('**/devices', { json: [] });
  const calls: NetworkCall[] = await route.calls();
  calls[0]?.answeredBy satisfies 'rule' | 'route' | 'mock' | 'real' | 'companion';
  const first: NetworkCall = await route.waitForCall({ timeout: 2_000 });
  first.method?.toUpperCase();
  first.status?.toFixed();
  const removed: void = await route.remove();
  const all: void = await t.app.network.removeAll();
  const spec: NetworkCall[] = await t.app.network.calls();
  const scenario = await t.app.mock.use({ rules: [{ http: 'GET **/x', json: {} }] }, undefined);
  const hit: NetworkCall = await scenario.waitForCall({ http: 'GET **/x' });
  const fn: NetworkCall = await scenario.waitForCall({ function: 'orders.submit' }, { timeout: 1_000 });
  hit.rule?.toFixed(); fn.function?.toUpperCase();
  const scenarioCalls: NetworkCall[] = await scenario.calls({ rule: 1 });
  // @ts-expect-error A target is an http rule, a function or a rule number.
  await scenario.waitForCall({ url: '**/x' });
  const gone: void = await scenario.remove();
  // @ts-expect-error The raw driver's `requests()` is `calls()` on the fixture.
  await route.requests();
  void removed; void all; void spec; void scenarioCalls; void gone;
  // The test API selects no mocks: the session's selection applies, and
  // only a scenario state goes on top.
  // @ts-expect-error scenarios are `t.app.mock.use()`
  await t.app['scenario']({ rules: [] });
  // @ts-expect-error `lxdev mock` and `lingxia dev --mock` select, not specs
  await t.app.mock.all();
  // @ts-expect-error each spec starts fresh already
  await t.app.mock.reset();
});

// The fixture app has its own shapes (view, logic, calls, clock state,
// checkpoints) and is not a raw `LxAppDriver`; its view is not a raw page.
declare const fixtureApp: TestApp;
// @ts-expect-error The view has locators, not the raw page methods.
const asRawPage: PageDriver = fixtureApp.view;
// @ts-expect-error Fixture network, clock and profile resolve their own shapes.
const asRawApp: LxAppDriver = fixtureApp;
void asRawApp; void asRawPage;

spec.fail('known inactive page', { expected: { code: 'E_PAGE_NOT_ACTIVE' } }, async () => {});
spec.fail('known timeout', { expected: { code: 'E_TIMEOUT' } }, async () => {});
spec.fail('known contract break', { expected: { code: 'E_OPENAPI_CONTRACT' } }, async () => {});
const knownCode: AutomationErrorCode = AUTOMATION_ERROR_CODES[0];
// @ts-expect-error Automation codes are a closed union.
const mistypedCode: AutomationErrorCode = 'E_PAGE_INACTIVE';
const testCodes: readonly TestErrorCode[] = TEST_ERROR_CODES;
const timeoutCode: 'E_TIMEOUT' = new TimeoutError('x').code;
// @ts-expect-error A driver timeout reaches a spec as E_TIMEOUT.
const driverTimeout: TestErrorCode = 'E_EVAL_TIMEOUT';
void driverTimeout;
const asTestCode: TestErrorCode = knownCode;
const skipped: TestErrorCode = 'E_SKIPPED';
// @ts-expect-error Test error codes are a closed union.
const mistypedTestCode: TestErrorCode = 'E_TIMED_OUT';
void testCodes; void timeoutCode; void asTestCode; void skipped; void mistypedTestCode;
declare const failure: FailureRecord;
failure.page?.instanceId?.toUpperCase();

// Test clock and rollback that keeps chosen storage keys.
spec('clock', { restoreProfile: { keep: ['auth.*'] } }, async (t) => {
  const started: ClockState = await t.app.clock.install({ now: new Date(0) });
  started.now.toFixed(); started.pending.toFixed();
  const { now, fired, pending }: ClockAdvance = await t.app.clock.tick(3_000);
  const ran: ClockAdvance = await t.app.clock.runAll({ maxTimers: 10 });
  const set: ClockState = await t.app.clock.setSystemTime(Date.now());
  const off: void = await t.app.clock.uninstall();
  const checkpoint: ProfileCheckpoint = await t.app.profile.checkpoint();
  const { kept } = await t.app.profile.restore(checkpoint, { keep: ['auth.*'] });
  kept.map((key: string) => key.toUpperCase());
  await t.app.profile.restore(checkpoint.id);
  const dropped: void = await t.app.profile.drop(checkpoint);
  // @ts-expect-error tick takes milliseconds.
  await t.app.clock.tick('3s');
  // @ts-expect-error install resolves a ClockState, not a number.
  const asNumber: number = await t.app.clock.install();
  void now; void fired; void pending; void ran; void set; void off; void dropped; void asNumber;
});
// @ts-expect-error keep is a list of globs.
spec('bad keep', { restoreProfile: { keep: 'auth.*' } }, async () => {});
// Tags: a file default plus the spec's own; report rows are typed.
spec.configure({ tags: ['routed'] });
spec('tagged spec', { tags: ['smoke'], covers: ['DEV-1'] }, async () => {});
// @ts-expect-error Tags are a list of strings.
spec('bad tags', { tags: 'smoke' }, async () => {});
declare const tagRow: TagSummary;
const tagOk: boolean = tagRow.ok;
void tagOk;

// Contract assertions take a schema name, a ref, or a ref in a named document.
expect({ id: 'd1' }).toMatchSchema('Device');
expect({ id: 'd1' }).not.toMatchSchema({ ref: '#/components/schemas/Device', document: 'api.yaml' });
// @ts-expect-error A schema target is a name or a ref, not a schema object.
expect({}).toMatchSchema({ type: 'object' });
declare const contractReport: JsonReport;
contractReport.openapi?.routed.failed.toFixed();
contractReport.coverage?.uncovered.map((entry) => entry.id);
// A contract-only spec declares it and is skipped without --openapi.
spec('contract only', { requires: { openapi: true } }, async (t) => {
  t.openapi?.documents.map((doc) => doc.name.toUpperCase());
});
// File defaults: every SpecOptions key but `id`, plus `requires`.
spec.configure({ timeout: 60_000, fresh: true, requires: { args: ['PASSWORD'] }, forensics: false, covers: ['DEV-2'] });
// @ts-expect-error Ids are per spec.
spec.configure({ id: 'shared' });
// @ts-expect-error requires.args lists --arg keys.
spec('bad requires', { requires: { args: 'PASSWORD' } }, async () => {});

// Host tiers read lazily: the read never throws, a call rejects.
spec('host tiers', async (t) => {
  await t.automation.desktop.window.status({} as never);
  await t.automation.terminal.snapshot({ surfaceId: 's' } as never);
  await t.automation.browser.tabs();
});


// The raw root is an import with host authority, never an ambient `lx`.
const rawRoot = rawAutomation();
void rawRoot.lxapp('example').network;
void rawRoot.lxapps.list();
// @ts-expect-error The test program has no ambient `lx`; import rawAutomation instead.
void lx.automation();
// The test context's runtime globals stay typed without app types.
void setTimeout(() => {}, 1);
void fetch;
void console;

// The main entry is for writing specs; running them and reading reports have their own.
void run; void trackPublicSurface; VERSION.toUpperCase();
// @ts-expect-error The runner is `@lingxia/test/runner`.
import { run as mainRun } from '../dist/index.js';
void mainRun;
