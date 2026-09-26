import { currentPageOrNull, waitForCurrentPage } from '../helpers/page.js';
import { expect, rawAutomation, spec } from '@lingxia/test';
import { bindFixture, expectReject, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import type { ProbeDocument, ProbeElement } from '../helpers/view.js';

spec("expose only host automation authority in the test runtime", { id: "AUT-000", covers: ['lx.automation'], app: SHOWCASE_APP_ID }, (t) => {
    const testLx = lx as unknown as Record<string, unknown>;
    const scope = globalThis as Record<string, unknown>;
    expect(typeof testLx.automation).toBe('function');
    expect(testLx.app).toBeUndefined();
    expect(testLx.env).toBeUndefined();
    expect(testLx.getStorage).toBeUndefined();
    expect(scope.App).toBeUndefined();
    expect(scope.Page).toBeUndefined();
    expect(scope.getApp).toBeUndefined();
    expect(scope.getCurrentPages).toBeUndefined();
    expect(scope.process).toBeUndefined();
    expect(scope.window).toBeUndefined();
    expect(scope.document).toBeUndefined();
  });

spec("select and inspect the current lxapp", { id: "AUT-001", covers: ['Automation.lxapp', 'LxAppDriver.info', 'LxAppDriver.pages'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "AUT-001");

    const info = await app.info();
    const pages = await app.pages();

    expect(info.appid).toBe('lingxia-showcase');
    expect(pages.some((page) => page.name === 'todo')).toBeTruthy();
  });

spec("reject re-entrant self-eval from the app Logic runtime", { id: "AUT-005", covers: ['LxAppDriver.eval'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "AUT-005");

    const rejection = await app.logic.eval({ timeout: 15_000 }, async ({ lx }) => {
      // The raw driver's `{ script }` eval is what this spec is about.
      try {
        await rawAutomation().lxapp().eval({ script: 'true', timeoutMs: 1_000 });
        return { rejected: false };
      } catch (error) {
        const failure = error as { code?: string; message?: string } | null;
        return {
          rejected: true,
          code: String(failure?.code || ''),
          message: String(failure?.message || error),
        };
      }
    });

    expect(rejection.rejected).toBeTruthy();
    expect(rejection.code).toBe('E_AUTOMATION');
    expect(rejection.message).toContain('cannot eval the calling app');
  });

spec("evaluate across the Logic boundary", { id: "AUT-002", covers: ['LxAppDriver.eval'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "AUT-002");

    // The raw driver's `{ script }` eval is what this spec covers.
    expect(await rawAutomation().lxapp(SHOWCASE_APP_ID).eval({ script: '21 * 2' })).toBe(42);
  });

spec("read the host surface plan with JavaScript-shaped fields", { id: "AUT-003", covers: ['LxAppDriver.surfaceLayout'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "AUT-003");

    const layout = await app.surfaceLayout();
    const rootId = layout.mainSwitcher.rootSurfaceId;
    if (rootId !== undefined) {
      const root = layout.mainSwitcher.items.find((item) => item.surfaceId === rootId);
      expect(root?.root).toBeTruthy();
      if (root?.content.kind !== 'lxapp') {
        throw new Error(`expected an lxapp root, got ${root?.content.kind ?? 'missing'}`);
      }
      expect(root.content.appId).toBe('lingxia-showcase');
    } else {
      expect(layout.mainSwitcher.items.some((item) => item.root)).toBeFalsy();
    }
    const serialized = JSON.stringify(layout);
    expect(serialized.includes('"app_id"')).toBeFalsy();
    expect(serialized.includes('"surface_id"')).toBeFalsy();
    expect(serialized.includes('"active_id"')).toBeFalsy();
  });

spec("wait for every page element state", { id: "AUT-004", covers: ['PageDriver.waitFor'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, namespace, defer } = bindFixture(t, "AUT-004");

    const current = await currentPageOrNull(app);
    if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPage(app, 'home');
    await app.view.testId('home-page', { page: 'home' }).waitFor({ state: 'visible', timeout: 30_000 });
    // The raw page driver's own wait states are what this spec covers.
    const page = rawAutomation().lxapp(SHOWCASE_APP_ID).page;

    const id = `automation-wait-${namespace}`;
    const css = `#${id}`;
    await app.view.eval({ page: 'home' }, ({ document }, id) => {
      const doc = document as unknown as ProbeDocument;
      const fixture = doc.createElement('input') as ProbeElement & { id: string; type: string; disabled: boolean };
      fixture.id = id;
      fixture.type = 'text';
      fixture.style.cssText = 'display:none;position:fixed;left:20px;top:20px;width:120px;height:32px;z-index:2147483647';
      fixture.disabled = true;
      doc.body.appendChild(fixture);
    }, id);
    const setFixture = (change: 'show' | 'enable' | 'remove') => app.view.eval({ page: 'home' }, ({ document }, id, change) => {
      const fixture = document.getElementById(id) as (ProbeElement & { disabled: boolean }) | null;
      if (change === 'show' && fixture) fixture.style.display = 'block';
      else if (change === 'enable' && fixture) fixture.disabled = false;
      else if (change === 'remove') fixture?.remove();
    }, id, change);
    defer(async () => {
      await setFixture('remove');
    });

    await page.waitFor({ page: 'home', css, state: 'attached' });
    await page.waitFor({ page: 'home', css, state: 'hidden' });
    await expectReject(
      () => page.waitFor({ page: 'home', css, state: 'enabled', timeoutMs: 100 }),
      { message: 'E_TIMEOUT' });
    await expectReject(
      () => page.waitFor({ page: 'not-a-showcase-page', css, state: 'attached' }),
      { message: 'unknown page name' });
    const visible = page.waitFor({ page: 'home', css, state: 'visible' });
    await setFixture('show');
    await visible;
    const enabled = page.waitFor({ page: 'home', css, state: 'enabled' });
    await setFixture('enable');
    await enabled;
    await page.waitFor({ page: 'home', css, state: 'editable' });
    const detached = page.waitFor({ page: 'home', css, state: 'detached' });
    await setFixture('remove');
    await detached;
  });
