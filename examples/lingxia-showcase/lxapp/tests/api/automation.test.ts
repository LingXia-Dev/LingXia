import { expect } from '@rongjs/test';
import { waitForCurrentPage } from '../helpers/page.js';
import { contract, expectReject } from '../support/contract.js';

contract(
  {
    id: 'AUT-000',
    title: 'expose only host automation authority in the test runtime',
    covers: ['automation runtime isolation'],
    layer: 'automation',
    levels: ['failure', 'boundary'],
    scope: 'portable',
    expectedOutcome: 'absent',
  },
  () => {
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
  },
);

contract(
  {
    id: 'AUT-001',
    title: 'select and inspect the current lxapp',
    covers: ['Automation.lxapp', 'LxAppDriver.info', 'LxAppDriver.pages'],
    layer: 'automation',
    levels: ['semantic'],
    scope: 'portable',
    expectedOutcome: 'supported',
  },
  async ({ app }) => {
    const info = await app.info();
    const pages = await app.pages();

    expect(info.appid).toBe('lingxia-showcase');
    expect(pages.some((page) => page.name === 'todo')).toBeTruthy();
  },
);

contract(
  {
    id: 'AUT-005',
    title: 'reject re-entrant self-eval from the app Logic runtime',
    covers: ['LxAppDriver.eval authorization'],
    layer: 'automation',
    levels: ['failure'],
    scope: 'portable',
    expectedOutcome: 'reject',
  },
  async ({ app }) => {
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
  },
);

contract(
  {
    id: 'AUT-002',
    title: 'evaluate across the Logic boundary',
    covers: ['LxAppDriver.eval'],
    layer: 'automation',
    levels: ['semantic', 'boundary'],
    scope: 'portable',
    expectedOutcome: 'supported',
  },
  async ({ app }) => {
    expect(await app.eval({ script: '21 * 2' })).toBe(42);
  },
);

contract(
  {
    id: 'AUT-003',
    title: 'read the host surface plan with JavaScript-shaped fields',
    covers: ['LxAppDriver.surfaceLayout'],
    layer: 'automation',
    levels: ['semantic', 'boundary'],
    scope: 'portable',
    expectedOutcome: 'supported',
  },
  async ({ app }) => {
    const layout = await app.surfaceLayout();
    const rootId = layout.mainSwitcher.rootSurfaceId;
    const root = layout.mainSwitcher.items.find((item) => item.surfaceId === rootId);

    expect(rootId).toBe('lingxia-showcase');
    expect(root?.root).toBeTruthy();
    if (root?.content.kind !== 'lxapp') {
      throw new Error(`expected an lxapp root, got ${root?.content.kind ?? 'missing'}`);
    }
    expect(root.content.appId).toBe('lingxia-showcase');
    const serialized = JSON.stringify(layout);
    expect(serialized.includes('"app_id"')).toBeFalsy();
    expect(serialized.includes('"surface_id"')).toBeFalsy();
    expect(serialized.includes('"active_id"')).toBeFalsy();
  },
);

contract(
  {
    id: 'AUT-004',
    title: 'wait for every page element state',
    covers: ['PageDriver.waitFor'],
    layer: 'automation',
    levels: ['semantic', 'failure', 'lifecycle'],
    scope: 'portable',
    expectedOutcome: 'mixed',
  },
  async ({ app, namespace, defer }) => {
    await app.nav.relaunch({ page: 'home' });
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
      { message: 'E_TIMEOUT' },
    );
    await expectReject(
      () => app.page.waitFor({ page: 'not-a-showcase-page', css, state: 'attached' }),
      { message: 'unknown page name' },
    );
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
  },
);
