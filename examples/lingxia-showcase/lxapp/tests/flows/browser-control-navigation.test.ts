import { expect, spec } from '@lingxia/test';

// Run against a host registering Settings and Downloads, with the device awake,
// unlocked and host foregrounded. Override URLs/selectors/RPC via --arg for a
// different host; the clicks must target real product links, not injected links.
spec('control-page links retain trusted bridge authority', {
  id: 'BROWSER-CONTROL-NAV-001',
  timeout: 180_000,
}, async (t) => {
  const browser = t.automation.browser;
  const fromUrl = t.args.fromUrl || 'lingxia://settings#downloads';
  const toUrl = t.args.toUrl || 'lingxia://downloads';
  const forward = t.args.forwardSelector || 'a[href="lingxia://downloads"]';
  const back = t.args.backSelector || 'a[href="lingxia://settings#downloads"]';
  const rpc = t.args.rpc || 'downloads.getSettings';
  const cycles = Number(t.args.cycles || 10);
  if (!Number.isInteger(cycles) || cycles < 1 || cycles > 30) {
    throw new Error('cycles must be an integer from 1 to 30');
  }
  const before = await browser.tabs();
  // Internal browser pages require the sealed host entrypoint. browser.open
  // deliberately rejects lingxia:// URLs and must not be used as a shortcut.
  await t.app.eval({ script: "return lx.shell.openBuiltin('downloads')" });
  await t.expect(async () => (await browser.current())?.current_url,
    { timeout: 12_000 }).toContain('lingxia://downloads');
  const opened = await browser.current();
  if (!opened) throw new Error('host builtin did not open a browser tab');
  const tab = opened.tab_id;
  await browser.activate({ tab });
  t.defer(async () => {
    if (!before.some((item) => item.tab_id === tab)) await browser.close({ tab });
  });
  const evidence: unknown[] = [];
  t.defer(() => t.attach('control-navigation.json', evidence));

  async function ready(url: string): Promise<void> {
    await browser.wait({
      tab,
      js: `document.readyState === 'complete' && location.href.split('#')[0].replace(/\\/$/, '') === ${JSON.stringify(url.split('#')[0].replace(/\/$/, ''))} && !!window.LingXiaBridge && window.LingXiaBridge.isReady()`,
      timeoutMs: 12_000,
    });
    const result = await browser.eval<{ ok: boolean; url: string; error?: string }>({
      tab,
      js: `Promise.race([
        window.LingXiaBridge.invoke(${JSON.stringify(rpc)}).then(function(value) {
          return { ok: !!value && typeof value === 'object', url: location.href };
        }),
        new Promise(function(_, reject) { setTimeout(function() { reject(new Error('bridge RPC timed out')); }, 5000); })
      ]).catch(function(error) { return { ok: false, url: location.href, error: String(error) }; })`,
    });
    evidence.push(result);
    expect(result.ok).toBe(true);
    const current = await browser.current();
    expect(current?.tab_id).toBe(tab);
    await browser.wait({ tab, js: `document.visibilityState === 'visible'`, timeoutMs: 5000 });
  }

  await ready(toUrl);
  await browser.click({ tab, css: back, waitNavigation: true, complete: true, timeoutMs: 12_000 });
  await ready(fromUrl);
  for (let i = 0; i < cycles; i += 1) {
    await t.step(`cycle ${i + 1}: Settings → Downloads → settings gear`, async () => {
      await browser.click({ tab, css: forward, waitNavigation: true, complete: true, timeoutMs: 12_000 });
      await ready(toUrl);
      await browser.click({ tab, css: back, waitNavigation: true, complete: true, timeoutMs: 12_000 });
      await ready(fromUrl);
    });
  }
  await t.step('reload still admits the control document', async () => {
    await browser.eval({ tab, js: `window.__controlNavigationOldDocument = true` });
    await browser.reload({ tab });
    await browser.wait({ tab, js: `!window.__controlNavigationOldDocument`, timeoutMs: 12_000 });
    await ready(fromUrl);
  });
});
