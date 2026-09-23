import { spec, type TestApp } from '../dist/index.js';
import type { Automation, LxAppDriver, PageDriver, PageQueryResult } from '@lingxia/types/automation';

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
  await app.nav.back({waitUntil:'ready', timeoutMs:5_000});
  // @ts-expect-error The nav option is `waitUntil`; `waitFor` is the page/locator method.
  await app.nav.to({page:'editor', waitFor:'ready'});
  await input.waitFor({state:'attached'});
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
  const count: number = await t.app.eval(({ getCurrentPages }) => getCurrentPages().length);
  count.toFixed();
  const route: string = await t.app.eval(async ({ getCurrentPages }, index: number) => {
    const pages = getCurrentPages();
    return pages[pages.length - 1 - index].route;
  }, 0);
  route.toUpperCase();
  const envPath = await t.app.eval(({ lx }) => lx.env.USER_DATA_PATH);
  envPath.valueOf();
  // @ts-expect-error Logic API members are checked against the published `lx`.
  await t.app.eval(({ lx }) => lx.noSuchApi());
  // @ts-expect-error Arguments must be JSON values; functions do not cross the boundary.
  await t.app.eval((_scope, callback: () => void) => callback(), () => {});
  // @ts-expect-error Arguments are checked against the function's parameters.
  await t.app.eval((_scope, id: string) => id, 42);
  // The string form still works and keeps its declared result type.
  const ready = await t.app.eval<boolean>({ script: 'return true' });
  ready.valueOf();

  // Page (WebView) eval: without the DOM lib, `document` is unknown and needs a cast.
  const title = await t.app.page.eval(({ document }) => (document as { title: string }).title);
  title.toUpperCase();
  // @ts-expect-error Without the DOM lib, `document` is unknown.
  await t.app.page.eval(({ document }) => document.title);
  const raw = await t.app.page.eval<number>({ script: '1 + 1' });
  raw.toFixed();

  interface Devices { devices: { id: string }[] }
  const data = await t.app.pageData<Devices>({ page: 'devices' });
  data.devices[0].id.toUpperCase();
  const renamed = await t.app.callPage<boolean>('rename', 'dev-1', { name: 'Office' });
  renamed.valueOf();
  // @ts-expect-error callPage arguments must be JSON values.
  await t.app.callPage('rename', undefined);

  // waitFor resolves to the accepted value.
  const loaded: Devices = await t.waitFor(() => t.app.pageData<Devices>(), (d) => d.devices.length > 0, { timeout: 2_000 });
  loaded.devices.length.toFixed();
  const truthy: string = await t.waitFor(async () => 'ok');
  truthy.toUpperCase();
  // @ts-expect-error accept receives the read value.
  await t.waitFor(() => 1, (value: string) => value.length > 0);

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

// Fixture drivers stay assignable to the raw drivers, so shared helpers typed
// against `@lingxia/types/automation` keep accepting them.
declare const fixtureApp: TestApp;
const asRawApp: LxAppDriver = fixtureApp;
const asRawPage: PageDriver = fixtureApp.page;
void asRawApp; void asRawPage;
