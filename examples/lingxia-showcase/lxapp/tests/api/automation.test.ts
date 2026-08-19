import { currentPageOrNull, waitForCurrentPage } from '../helpers/page.js';
import { expect, spec } from '@lingxia/test';
import { bindFixture, expectReject, specNamespace } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

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

    const rejection = await app.eval({
      timeoutMs: 15_000,
      script: `
        try {
          await lx.automation().lxapp().eval({ script: 'true', timeoutMs: 1_000 });
          return { rejected: false };
        } catch (error) {
          return {
            rejected: true,
            code: String(error?.code || ''),
            message: String(error?.message || error),
          };
        }
      `,
    }) as { rejected: boolean; code?: string; message?: string };

    expect(rejection.rejected).toBeTruthy();
    expect(rejection.code).toBe('E_AUTOMATION');
    expect(rejection.message).toContain('cannot eval the calling app');
  });

spec("evaluate across the Logic boundary", { id: "AUT-002", covers: ['LxAppDriver.eval'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "AUT-002");

    expect(await app.eval({ script: '21 * 2' })).toBe(42);
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
    await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]', state: 'visible' });

    const id = `automation-wait-${namespace}`;
    const css = `#${id}`;
    await app.page.eval({
      page: 'home',
      script: `
        const fixture = document.createElement('input');
        fixture.id = ${JSON.stringify(id)};
        fixture.type = 'text';
        fixture.style.cssText = 'display:none;position:fixed;left:20px;top:20px;width:120px;height:32px;z-index:2147483647';
        fixture.disabled = true;
        document.body.appendChild(fixture);
      `,
    });
    defer(async () => {
      await app.page.eval({
        page: 'home',
        script: `document.getElementById(${JSON.stringify(id)})?.remove()`,
      });
    });

    await app.page.waitFor({ page: 'home', css, state: 'attached' });
    await app.page.waitFor({ page: 'home', css, state: 'hidden' });
    await expectReject(
      () => app.page.waitFor({ page: 'home', css, state: 'enabled', timeoutMs: 100 }),
      { message: 'E_TIMEOUT' });
    await expectReject(
      () => app.page.waitFor({ page: 'not-a-showcase-page', css, state: 'attached' }),
      { message: 'unknown page name' });
    const visible = app.page.waitFor({ page: 'home', css, state: 'visible' });
    await app.page.eval({
      page: 'home',
      script: `document.getElementById(${JSON.stringify(id)}).style.display = 'block'`,
    });
    await visible;
    const enabled = app.page.waitFor({ page: 'home', css, state: 'enabled' });
    await app.page.eval({
      page: 'home',
      script: `document.getElementById(${JSON.stringify(id)}).disabled = false`,
    });
    await enabled;
    await app.page.waitFor({ page: 'home', css, state: 'editable' });
    const detached = app.page.waitFor({ page: 'home', css, state: 'detached' });
    await app.page.eval({
      page: 'home',
      script: `document.getElementById(${JSON.stringify(id)})?.remove()`,
    });
    await detached;
  });
