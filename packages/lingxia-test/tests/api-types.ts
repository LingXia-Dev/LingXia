import {
  spec, expect, TEST_ERROR_CODES, TimeoutError, type JsonValue, type AnyLogicPage, type Fixture, type AutomationErrorCode, type ClockAdvance, type ClockState, type NetworkCall, type PageContract, type ProfileCheckpoint, type TestApp, type TestErrorCode, type TestPage, type TestView, type UnaryActionsOnly, type ViewElement,
} from '../dist/index.js';
import type { FailureRecord, JsonReport, TagSummary } from '@lingxia/test/report';
import { run, rawAutomation, trackPublicSurface, VERSION } from '@lingxia/test/runner';
import { AUTOMATION_ERROR_CODES, type Automation, type LxAppDriver, type PageDriver } from '@lingxia/types/automation';

spec('typed test boundary', async t => {
  const app: TestApp = t.automation.lxapp('example');
  const editor: TestPage<PageContract> = await app.page({ name: 'editor' }, { timeout: 2_000 });
  editor.instanceId.toUpperCase(); editor.name.toUpperCase();
  const editorView: TestView = editor.view;
  const input = editorView.testId('input').nth(0);
  await input.press('Enter');
  // One-shot reads: no wait, no retry.
  const count: number = await input.count();
  const visible: boolean = await input.isVisible();
  const text: string = await input.textContent();
  const value: string = await input.inputValue();
  const label: string | null = await input.getAttribute('aria-label');
  void count; void visible; void text; void value; void label;
  // @ts-expect-error `query()` is gone: read with count/isVisible/textContent/inputValue/getAttribute.
  await input.query();
  // @ts-expect-error getAttribute needs a name.
  await input.getAttribute();
  const state = await app.logic.eval(() => ({ ready: true }));
  state.ready.valueOf();
  // @ts-expect-error The fixture has no string eval; a script string is for the raw driver.
  await app.eval({script:'1'});
  await t.automation.browser.tabs();
  const landed = await app.nav.to({page:'editor', waitUntil:'commit'});
  landed.webviewAttached.valueOf();
  landed.instanceId?.toUpperCase();
  await t.reject(async () => (await app.page({ name: 'devices' })).view.css('#save').click(), {code:'E_PAGE_NOT_ACTIVE'});
  // @ts-expect-error Locators take no page option: bind the page with `app.page({ name })`.
  app.view.css('#save', {page:'devices'});
  // @ts-expect-error Nor a match index: use `.nth(i)`.
  app.view.testId('row', {index:1});
  // @ts-expect-error A page selector is a name or an instance id, not both.
  await app.page({ name: 'editor', instanceId: '1' });
  // @ts-expect-error `app.page()` waits with `timeout`.
  await app.page({ name: 'editor' }, { timeoutMs: 1_000 });
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
  // @ts-expect-error `t.app.page` binds a page; its view holds the locators.
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
  // @ts-expect-error Reads are one-shot; they do not accept ignored retry options.
  input.count({timeout:100});
  // @ts-expect-error Screenshots take no page option; bind the page.
  await app.view.screenshot({page:'editor'});
  // @ts-expect-error Nor does scroll.
  await app.view.scroll({page:'editor', dy:100});
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
  const surface = await t.app.page({ name: 'surface' });
  const surfaceTitle: string = await surface.view.eval(({ document }, suffix: string) => document.title + suffix, '!');
  surfaceTitle.toUpperCase();
  const slow: number = await t.app.logic.eval({ timeout: 30_000 }, ({ lx }, n: number) => n + lx.env.USER_DATA_PATH.length, 1);
  slow.toFixed();
  await surface.view.eval({ timeout: 20_000 }, ({ document }) => document.title);
  // @ts-expect-error `page` is not an eval option: bind the page and use its view.
  await t.app.view.eval({ page: 'surface', timeout: 20_000 }, ({ document }) => document.title);
  // @ts-expect-error Eval options are `{ timeout }`.
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
  // @ts-expect-error Native keyboard input has no page target.
  (await t.app.page({ name: 'cart' })).view.key;
  t.app.window.key;
  t.app.window.pointer;
  const cart = (await t.app.page({ name: 'cart' })).view;
  await cart.testId('total').click();
  const cartTitle: string = await cart.eval(({ document }) => document.title);
  cartTitle.toUpperCase();
  // @ts-expect-error A bound View cannot retarget another page.
  cart.page('checkout');
  await (await t.app.page({ name: 'checkout' })).view.screenshot();
  // @ts-expect-error app.page() is the only page binding entrypoint.
  t.app.view.page('cart');

  // Invalid results fail at the call site, not when a `never` is used later.
  const shaped = await t.app.logic.eval(() => ({ id: 'a', at: 1, tags: ['x'] as const }));
  shaped.id.toUpperCase(); shaped.tags[0].toUpperCase();
  // @ts-expect-error Methods cannot cross the JSON boundary.
  await t.app.logic.eval(() => ({ save() {} }));
  // @ts-expect-error Convert a Date explicitly before returning it.
  await t.app.logic.eval(() => new Date());
  // @ts-expect-error Return element data rather than a DOM handle.
  await t.app.view.eval(({ document }) => document.body);
  // @ts-expect-error Nested undefined would silently become null.
  await t.app.logic.eval(() => [undefined]);
  interface Payload { id: string; items: readonly number[] }
  const payload: Payload = { id: 'a', items: [1] };
  const echoed = await t.app.logic.eval((_, value: Payload) => value, payload);
  echoed.id.toUpperCase();
  interface TreeNode { nodeType: number; label: string }
  const node: TreeNode = { nodeType: 1, label: 'folder' };
  const logicNode: TreeNode = await t.app.logic.eval((_, value: TreeNode) => value, node);
  const viewNode: TreeNode = await t.app.view.eval(() => ({ nodeType: 1, label: 'folder' }));
  logicNode.nodeType.toFixed(); viewNode.label.toUpperCase();
  // @ts-expect-error A DOM handle is still not a JSON argument.
  await t.app.logic.eval((_, element: ViewElement) => element.id, {} as ViewElement);
  const anyValue = await t.app.logic.eval(() => JSON.parse('1') as any);
  anyValue.whatever;
  const maybe = await t.app.view.eval(({ document }) => document.querySelector('x')?.textContent ?? undefined);
  maybe?.toUpperCase();
  interface Devices { devices: { id: string }[] }
  // A page contract types the bound page's data and its public actions.
  type DevicesContract = PageContract<Devices, {
    rename(patch: { id: string; name: string }): Promise<boolean>;
    count(): number;
  }>;
  const devices = await t.app.page<DevicesContract>({ name: 'devices' });
  const data = await devices.data();
  data.devices[0].id.toUpperCase();
  const done: boolean = await devices.actions.rename({ id: 'dev-1', name: 'Office' });
  done.valueOf();
  const counted: number = await devices.actions.count();
  counted.toFixed();
  // @ts-expect-error The action must be one the contract declares.
  await devices.actions.remove();
  // @ts-expect-error The declared action takes no payload.
  await devices.actions.count(1);
  // An untyped page reads as `unknown` data.
  const untypedData = await (await t.app.page({ instanceId: '7' })).data();
  // @ts-expect-error Untyped data is `unknown` until a contract types it.
  untypedData.devices;
  // @ts-expect-error `logic.data` is gone: `(await t.app.page()).data()`.
  await t.app.logic.data();
  // @ts-expect-error `logic.call` is gone: `page.actions.<name>(payload)`.
  await t.app.logic.call('rename', 'dev-1');
  const untyped: AnyLogicPage = null as unknown as AnyLogicPage;
  void untyped.anything;
  // `getPage` may find nothing: the instance is gone.
  const gone: boolean = await t.app.logic.eval(({ getPage }, id: string) => getPage(id) === undefined, '7');
  void gone;
  // @ts-expect-error getPage can return undefined.
  await t.app.logic.eval(({ getPage }, id: string) => getPage(id).route, '7');

  // waitFor resolves to the accepted value; the test is `until`.
  const loaded: Devices = await t.waitFor(() => devices.data(), { until: (d) => d.devices.length > 0, timeout: 2_000 });
  loaded.devices.length.toFixed();
  const truthy: string = await t.waitFor(async () => 'ok');
  truthy.toUpperCase();
  // @ts-expect-error until receives the read value.
  await t.waitFor(() => 1, { until: (value: string) => value.length > 0 });
  // @ts-expect-error No positional callback: pass { until }.
  await t.waitFor(() => 1, (value: number) => value > 0);

  // One expect: a locator retries, a value checks once, expect.poll(read) retries.
  await expect(t.app.view.testId('save')).toBeVisible();
  await expect.poll(() => devices.data(), { timeout: 2_000 }).toEqual({ devices: [] });
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
  expect(devices.data()).toEqual({});
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
  // @ts-expect-error Routes are spec-scoped; each has `remove()`.
  await t.app.network.removeAll();
  const spec: NetworkCall[] = await t.app.network.calls();
  const scenario = await t.scenario.use({ rules: [{ http: 'GET **/x', json: {} }] }, undefined);
  const hit: NetworkCall = await scenario.waitForCall({ http: 'GET **/x' });
  const fn: NetworkCall = await scenario.waitForCall({ function: 'orders.submit' }, { timeout: 1_000 });
  hit.rule?.toFixed(); fn.function?.toUpperCase();
  const scenarioCalls: NetworkCall[] = await scenario.calls({ rule: 1 });
  // @ts-expect-error A target is an http rule, a function or a rule number.
  await scenario.waitForCall({ url: '**/x' });
  const gone: void = await scenario.remove();
  // @ts-expect-error The raw driver's `requests()` is `calls()` on the fixture.
  await route.requests();
  await t.scenario.use({ rules: [] }, { app: 'other' });
  // @ts-expect-error `app` names an lxapp id, not a driver.
  await t.scenario.use({ rules: [] }, { app: 1 });
  void removed; void spec; void scenarioCalls; void gone;
  // The test API selects no mocks: the session's selection applies, and
  // only a scenario state goes on top.
  // @ts-expect-error scenarios are `t.scenario.use()`
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
spec.configure({ timeout: 60_000, start: { page: 'home' }, requires: { args: ['PASSWORD'] }, forensics: false, covers: ['DEV-2'] });
spec('starts on a page', { start: { page: 'detail', query: { id: 'd1' } } }, async () => {});
// @ts-expect-error `fresh` is gone: `start: { page }` relaunches on that page.
spec('fresh', { fresh: true }, async () => {});
// @ts-expect-error `start` names a page.
spec('bad start', { start: 'home' }, async () => {});
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


// The raw root is a runner import with host authority, never an ambient `lx`.
const rawRoot = rawAutomation();
// @ts-expect-error `rawAutomation` is `@lingxia/test/runner`, not the authoring entry.
import { rawAutomation as mainRaw } from '../dist/index.js';
void mainRaw;
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


spec('a page contract binds one identity and checks public action payloads', async t => {
  type Contract = import('../dist/index.js').PageContract<
    { title: string }, { rename(input: { title: string }): Promise<boolean> }
  >;
  const page = await t.app.page<Contract>({ name: 'editor' });
  const title: string = (await page.data()).title;
  const done: boolean = await page.actions.rename({ title });
  void done;
  // @ts-expect-error The contract determines the payload.
  await page.actions.rename({ title: 1 });
  // @ts-expect-error A handle cannot retarget one operation.
  await page.view.eval({ page: 'other' }, () => true);
  // @ts-expect-error No private method appears on the public action contract.
  await page.actions._save();
  // @ts-expect-error A handle's view takes no page option either.
  page.view.testId('x', { page: 'other' });

  // An action with more than one parameter is not callable: it takes one payload.
  type Wide = PageContract<{ n: number }, {
    move(from: number, to: number): void;
    feed(): AsyncGenerator<number>;
    optional(input?: { n: number }): number;
    stamp(at: Date): void;
  }>;
  const wide = await t.app.page<Wide>();
  const branded: UnaryActionsOnly = wide.actions.move;
  void branded;
  // @ts-expect-error UnaryActionsOnly: change the action to take an object.
  await wide.actions.move(1, 2);
  // @ts-expect-error A streamed (generator) action is not callable from a test.
  await wide.actions.feed();
  const optional: number = await wide.actions.optional();
  const withPayload: number = await wide.actions.optional({ n: 1 });
  void optional; void withPayload;
  // @ts-expect-error Payloads are JSON values.
  await wide.actions.stamp(new Date());

  // A contract whose data is not JSON has no data, and binding it fails.
  type NotJson = PageContract<{ at: Date }, {}>;
  // @ts-expect-error PageContract data must be JSON.
  await t.app.page<NotJson>();
  // @ts-expect-error Nor a function member.
  await t.app.page<PageContract<{ save(): void }, {}>>();
});

// Removed exports do not compile.
// @ts-expect-error Removed: use `page.actions`.
type RemovedCallOptions = import('../dist/index.js').LogicCallOptions;
// @ts-expect-error Removed: `page.data()` returns the contract's data.
type RemovedSnapshot = import('../dist/index.js').JsonSnapshot<{}>;
// @ts-expect-error Removed: a bound view is a `TestView`.
type RemovedBoundView = import('../dist/index.js').BoundTestView;
// @ts-expect-error Removed: locators take no options.
type RemovedLocatorOptions = import('../dist/index.js').LocatorOptions;
// @ts-expect-error Removed: eval options are `EvalOptions`.
type RemovedViewEvalOptions = import('../dist/index.js').ViewEvalOptions;
// @ts-expect-error Host-side types are not on the authoring entry.
type RemovedController = import('../dist/index.js').LingxiaTestController;
type RunnerController = import('@lingxia/test/runner').LingxiaTestController;
// @ts-expect-error `t.app.logic` has only `eval`.
type RemovedLogicData = import('../dist/index.js').TestLogic['data'];
// @ts-expect-error Nor `call`.
type RemovedLogicCall = import('../dist/index.js').TestLogic['call'];
// @ts-expect-error Nor does a locator have `query`.
type RemovedQuery = import('../dist/index.js').Locator['query'];
// @ts-expect-error Nor the network `removeAll`.
type RemovedRemoveAll = import('../dist/index.js').TestNetwork['removeAll'];
// @ts-expect-error Nor a spec `fresh`.
type RemovedFresh = import('../dist/index.js').SpecOptions['fresh'];
export type { RunnerController };

spec('recursive JSON types cross eval without excessive instantiation', async t => {
  const input: JsonValue = JSON.parse('{"nested":[{"ok":true}]}');
  const result: JsonValue = await t.app.logic.eval((_, value) => value, input);
  const list: JsonValue[] = await t.app.view.eval((_, value) => value, [input] as JsonValue[]);
  void result; void list;
});

