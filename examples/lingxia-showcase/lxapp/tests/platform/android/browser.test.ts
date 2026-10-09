import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const browserSpec = args.platform === 'android' && args.httpBase ? spec : spec.skip;

browserSpec('follow a link, traverse history, and scroll a browser page', {
  id: 'ANDROID-BROWSER-001',
  app: SHOWCASE_APP_ID,
  covers: ['BrowserDriver.open', 'BrowserDriver.tabs', 'BrowserDriver.click',
    'BrowserDriver.back', 'BrowserDriver.forward', 'BrowserDriver.wait',
    'BrowserDriver.scroll', 'BrowserDriver.scrollTo', 'BrowserDriver.close'],
  reason: 'requires Android and the local HTTP fixture',
}, async (t) => {
  const browser = t.automation.browser;
  const { tab } = await browser.open({ url: `${args.httpBase}/interaction` });
  t.defer(() => browser.close({ tab }));
  t.defer(async () => {
    await t.attach('browser-state', await browser.tabs());
    await t.attach('browser-document', await browser.eval({ tab,
      js: '({url:location.href,ready:document.readyState,html:document.documentElement.outerHTML.slice(0,2000)})',
    }).catch((error: unknown) => String(error)));
  });
  await browser.wait({ tab, visible: '#next' });
  await browser.click({ tab, css: '#next', waitNavigation: true, complete: true });
  expect((await browser.tabs()).find((entry) => entry.tab_id === tab)?.can_go_back).toBe(true);
  await browser.back({ tab });
  await browser.wait({ tab, js: "location.pathname === '/interaction' && document.readyState === 'complete' && !!document.querySelector('#next')" });
  expect((await browser.tabs()).find((entry) => entry.tab_id === tab)?.can_go_forward).toBe(true);
  await browser.forward({ tab });
  await browser.wait({ tab, js: "location.pathname === '/page/second' && document.readyState === 'complete' && !!document.querySelector('h1')" });
  await browser.back({ tab });
  await browser.wait({ tab, js: "location.pathname === '/interaction' && document.readyState === 'complete' && !!document.querySelector('#next')" });
  await browser.scroll({ tab, dy: 450 });
  await browser.wait({ tab, js: 'scrollY > 100' });
  await browser.scrollTo({ tab, css: '#bottom' });
  expect((await browser.query({ tab, css: '#bottom' })).visible).toBe(true);
});

browserSpec('round-trip browser cookies across a reload', {
  id: 'ANDROID-BROWSER-002',
  app: SHOWCASE_APP_ID,
  covers: ['BrowserDriver.cookies.set', 'BrowserDriver.cookies.list',
    'BrowserDriver.cookies.delete', 'BrowserDriver.reload', 'BrowserDriver.eval'],
  reason: 'requires Android and the local HTTP fixture',
}, async (t) => {
  const browser = t.automation.browser;
  const { tab } = await browser.open({ url: `${args.httpBase}/interaction` });
  t.defer(() => browser.close({ tab }));
  await browser.wait({ tab, loaded: true });
  const name = `lingxia_probe_${Date.now()}`;
  t.defer(() => browser.cookies.delete({ tab, name, domain: '127.0.0.1', path: '/' }));
  await browser.cookies.set({ tab, name, value: 'pixel', url: args.httpBase, path: '/' });
  expect((await browser.cookies.list({ tab })).find((cookie) => cookie.name === name)?.value).toBe('pixel');
  await browser.eval({ tab, js: 'globalThis.__cookieReloadMarker = true' });
  await browser.reload({ tab });
  await browser.wait({ tab, js: `globalThis.__cookieReloadMarker === undefined && document.readyState === 'complete' && document.cookie.includes('${name}=pixel')` });
  await browser.cookies.delete({ tab, name, domain: '127.0.0.1', path: '/' });
  expect(await browser.eval({ tab, js: `document.cookie.includes('${name}=')` })).toBe(false);
});
