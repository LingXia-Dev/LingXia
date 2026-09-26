import {
  spec, expect, TEST_ERROR_CODES, TimeoutError,
  type AnyLogicPage, type AutomationErrorCode, type ClockAdvance, type ClockState, type FailureRecord, type JsonReport,
  type LogicPage, type NetworkCall, type ProfileCheckpoint, type TagSummary, type TestApp, type TestErrorCode,
} from '../dist/index.js';
import { AUTOMATION_ERROR_CODES, type Automation, type LxAppDriver, type PageDriver, type PageQueryResult } from '@lingxia/types/automation';

spec('typed test boundary', async t => {
  const app: TestApp = t.automation.lxapp('example');
  const input = app.page.testId('input', {page:'editor'}).nth(0);
  await input.press('Enter');
  const element: PageQueryResult = await input.query();
  if (element.exists) element.rect.width.toFixed();
  const state = await app.eval<{ready:boolean}>({script:'return {ready:true}'});
  state.ready.valueOf();
  // @ts-expect-error The fixture unwraps the call trace; specs do not opt in.
  await app.eval({script:'1', captureCalls:true});
  await t.automation.browser.tabs();
  const landed = await app.nav.to({page:'editor', waitUntil:'commit'});
  landed.webviewAttached.valueOf();
  landed.instanceId?.toUpperCase();
  await t.reject(() => app.page.click({css:'#save', page:'devices'}), {code:'E_PAGE_NOT_ACTIVE'});
  await app.nav.back({waitUntil:'ready', timeoutMs:5_000});
  // @ts-expect-error The nav option is `waitUntil`; `waitFor` is the page/locator method.
  await app.nav.to({page:'editor', waitFor:'ready'});
  await input.waitFor({state:'attached'});
  await input.waitFor({state:'inViewport'});
  const rows = app.page.css('li').filter({hasText:/ready/i});
  await rows.first().click({force:true, timeout:1_000});
  await rows.last().fill('x', {force:true});
  await t.expect(rows.nth(1)).toBeInViewport();
  await t.expect(rows.first()).toContainText('ready');
  await t.expect(rows.first()).toHaveAttribute('aria-selected', 'true');
  await t.expect(rows.first()).not.toHaveAttribute('disabled');
  await app.page.click({css:'#save', force:true});
  // @ts-expect-error `type` has no forced mode.
  await input.type('x', {force:true});
  // @ts-expect-error filter needs hasText.
  app.page.css('li').filter({});
  if (!state.ready) t.skip('not ready');
  // @ts-expect-error Test context has no DOM.
  document.querySelector('button');
  // @ts-expect-error Queries read once; they do not accept ignored retry options.
  input.query({timeout:100});
  // @ts-expect-error Locators require an explicit page string.
  app.page.css('button', {page:123});
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

spec.fail('known quota failure', { expected: { code: 'E_QUOTA', message: /quota/ } }, async () => {});
// @ts-expect-error `expected` belongs to spec.fail only.
spec('plain spec', { expected: { code: 'E_QUOTA' } }, async () => {});

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
  // @ts-expect-error The fixture takes functions; a script string is for the raw driver.
  await t.app.logic.eval({ script: 'return true' });
  // 0.18's string form keeps working on `t.app.eval`, deprecated.
  const ready = await t.app.eval<boolean>({ script: 'return true' });
  ready.valueOf();
  // @ts-expect-error `t.app.eval` keeps only the deprecated string form.
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
  // @ts-expect-error ViewDocument is minimal: no DOM writes without the DOM lib.
  await t.app.view.eval(({ document }) => document.write('x'));
  // @ts-expect-error The view takes functions; a script string is for the raw driver.
  await t.app.view.eval({ script: '1 + 1' });
  // The deprecated `t.app.page` still takes both forms.
  const raw = await t.app.page.eval<number>({ script: '1 + 1' });
  raw.toFixed();
  const viaPage: string = await t.app.page.eval(({ document }) => document.title);
  viaPage.toUpperCase();
  // @ts-expect-error The view exposes locators, not raw element methods.
  await t.app.view.click({ css: '#save' });
  // @ts-expect-error The view exposes locators, not raw element methods.
  await t.app.view.waitFor({ css: '#save' });
  await t.app.view.screenshot();

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

  // One waiting ladder: t.expect(locator) and t.expect(fn) retry, t.expect(value) checks once.
  await t.expect(t.app.view.testId('save')).toBeVisible();
  await t.expect(() => t.app.logic.data<Devices>(), { timeout: 2_000 }).toEqual({ devices: [] });
  await t.expect(async () => 3).toBeGreaterThan(2);
  t.expect(3).toBe(3);
  t.expect({ id: 'd1' }).toMatchSchema('Device');
  // @ts-expect-error A once-check is synchronous: it returns no promise to await on.
  t.expect(3).toBe(3).then;
  // @ts-expect-error Locator matchers are not value matchers.
  await t.expect(t.app.view.testId('save')).toBe(1);
  // The deprecated alias keeps working.
  await t.expect.poll(() => 1).toBe(1);

  // Args may be missing; t.arg narrows or throws.
  // @ts-expect-error A missing arg is undefined, not a string.
  const unchecked: string = t.args.baseUrl;
  void unchecked;
  const baseUrl: string = t.arg('baseUrl');
  const mode: string = t.arg('mode', { default: 'mock' });
  const optional = t.arg('token', { required: false });
  // @ts-expect-error An optional arg may be undefined.
  optional.toUpperCase();
  void baseUrl; void mode;
});

// One call record across routes, network and scenarios; removers resolve void.
spec('network calls', async (t) => {
  const route = await t.app.network.route('**/devices', { json: [] });
  const calls: NetworkCall[] = await route.calls();
  calls[0]?.answeredBy satisfies 'rule' | 'route' | 'real' | 'companion';
  const first: NetworkCall = await route.waitForCall({ timeout: 2_000 });
  first.method?.toUpperCase();
  first.status?.toFixed();
  const removed: void = await route.unroute();
  const all: void = await t.app.network.unrouteAll();
  const spec: NetworkCall[] = await t.app.network.calls();
  const scenario = await t.app.scenario({ rules: [{ http: 'GET **/x', json: {} }] }, undefined);
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
});

// The deprecated `t.app.page` stays assignable to the raw page driver, so
// helpers typed against it keep accepting it. The fixture app has its own
// shapes (calls, clock state, checkpoints) and is not a raw `LxAppDriver`.
declare const fixtureApp: TestApp;
const asRawPage: PageDriver = fixtureApp.page;
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
  const checkpoint: ProfileCheckpoint = await t.profile.checkpoint();
  const { kept } = await t.profile.restore(checkpoint, { keep: ['auth.*'] });
  kept.map((key: string) => key.toUpperCase());
  await t.profile.restore(checkpoint.id);
  const dropped: void = await t.profile.drop(checkpoint);
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

// expect(locator) is refused at run time; the type still takes it, so the
// message is what guides.
spec('trap', async (t) => {
  expect(t.app.view.testId('x'));
});
