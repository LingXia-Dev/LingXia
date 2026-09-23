import { spec, type TestApp } from '../dist/index.js';
import type { Automation, PageQueryResult } from '@lingxia/types/automation';

spec('typed test boundary', async t => {
  const app: TestApp = t.automation.lxapp('example');
  const input = app.page.testId('input', {page:'editor'}).nth(0);
  await input.press('Enter');
  const element: PageQueryResult = await input.query();
  if (element.exists) element.rect.width.toFixed();
  const state = await app.eval<{ready:boolean}>({script:'return {ready:true}'});
  state.ready.valueOf();
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
