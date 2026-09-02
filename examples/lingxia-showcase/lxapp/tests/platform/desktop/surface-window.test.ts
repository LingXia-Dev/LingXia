import { expect, spec } from '@lingxia/test';
import type { DesktopWindowInfo } from 'lingxia-types/automation';
import { runtimePlatform } from '../../helpers/platform.js';
import { waitForElementAttribute } from '../../helpers/page.js';
import { bindFixture, eventually } from '../../helpers/poll.js';
import { showcaseApp, SHOWCASE_APP_ID } from '../../helpers/app.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const targetPlatform = testArgs.platform?.toLocaleLowerCase();
const selectedGate = testArgs.gate?.toLocaleLowerCase();

const FULL_DRAG_STRIP_HEIGHT = 28;

interface OpenedWindow {
  id: string;
  key: string | undefined;
  kind: string;
  realized: string;
  visible: boolean;
  alive: boolean;
  chrome: string;
}

interface SurfacePageSnapshot {
  text: string;
  topInset: number;
  showCount: number;
}

async function desktopPlatform(): Promise<string> {
  const app = showcaseApp();
  const actual = await runtimePlatform(app);
  if (!['macos', 'windows'].includes(actual)) {
    throw new Error(
      `surface window tests require macOS or Windows; got ${actual || 'unknown'}`,
    );
  }
  if (targetPlatform && targetPlatform !== actual) {
    throw new Error(`requested ${targetPlatform}, but the running showcase reports ${actual}`);
  }
  return actual;
}

async function closeKeyedSurface(app: ReturnType<typeof showcaseApp>, key: string): Promise<void> {
  await app.eval({
    timeoutMs: 15_000,
    script: `
      const handle = lx.surface.get(${JSON.stringify(key)});
      if (handle) await handle.close();
    `,
  });
}

async function openWindow(
  app: ReturnType<typeof showcaseApp>,
  chrome: 'system' | 'full',
  key: string,
): Promise<OpenedWindow> {
  return await app.eval({
    timeoutMs: 20_000,
    script: `
      const handle = await lx.surface.openPage('surface', {
        as: 'window',
        chrome: ${JSON.stringify(chrome)},
        key: ${JSON.stringify(key)},
        size: { width: 480, height: 640 },
        query: { fixture: ${JSON.stringify(key)}, chrome: ${JSON.stringify(chrome)} },
      });
      return {
        id: handle.id,
        key: handle.key,
        kind: handle.kind,
        realized: handle.realized,
        visible: handle.visible,
        alive: handle.alive,
        chrome: ${JSON.stringify(chrome)},
      };
    `,
  }) as OpenedWindow;
}

async function waitForSurfacePage(
  app: ReturnType<typeof showcaseApp>,
  fixture: string,
  expectedTopInset: number,
): Promise<SurfacePageSnapshot> {
  await app.page.waitFor({
    page: 'surface',
    css: '[data-testid="surface-page"]',
    state: 'visible',
    timeoutMs: 15_000,
  });
  return eventually(
    () => app.page.eval({
      page: 'surface',
      script: `(() => {
        const layout = window.lxPageChrome && window.lxPageChrome.layout;
        const show = document.querySelector('[data-testid="surface-show-count"]');
        return {
          text: document.body ? document.body.innerText : '',
          topInset: layout ? layout.topInset : -1,
          showCount: Number((show && show.textContent.trim()) || 0),
        };
      })()`,
    }) as Promise<SurfacePageSnapshot>,
    (snapshot) => (
      typeof snapshot?.text === 'string'
      && snapshot.text.includes('Surface Page')
      && snapshot.text.includes(fixture)
      && snapshot.topInset === expectedTopInset
      && snapshot.showCount >= 1
    ),
    {
      timeoutMs: 10_000,
      describe: `surface window page ${fixture} with topInset ${expectedTopInset}`,
    });
}

function newSurfaceWindow(
  before: DesktopWindowInfo[],
  after: DesktopWindowInfo[],
): DesktopWindowInfo | undefined {
  const known = new Set(before.map((window) => window.id));
  const requestedSizeDistance = (window: DesktopWindowInfo): number => (
    Math.abs(window.bounds.w - 480) + Math.abs(window.bounds.h - 640)
  );
  return after
    .filter((window) => (
      window.visible
      && !window.minimized
      && !known.has(window.id)
      && window.process.toLocaleLowerCase() !== 'msedgewebview2'
      && window.bounds.w >= 400
      && window.bounds.h >= 400
    ))
    // The surface window was just presented with activate: true, so it is the
    // frontmost of any new window; size proximity only breaks ties.
    .sort((left, right) => (
      left.z - right.z || requestedSizeDistance(left) - requestedSizeDistance(right)
    ))[0];
}

const windowTest = selectedGate ? spec.skip : spec;

windowTest('open a page window with system chrome and with full chrome', {
  id: 'DESKTOP-SURFACE-WINDOW-001',
  covers: [
    'lx.surface.openPage',
    'lx.surface.get',
    'lx.supports',
    'PageSurface.kind',
    'PageSurface.realized',
    'PageSurface.id',
    'PageSurface.key',
    'PageSurface.alive',
    'PageSurface.visible',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-WINDOW-001');
  const platform = await desktopPlatform();
  const desktop = lx.automation().desktop;
  const fullOffered = await app.eval({
    script: `return !!lx.supports({ capability: 'surface', value: 'window', chrome: 'full' })`,
  }) as boolean;
  expect(fullOffered).toBeTruthy();

  const chromes: Array<'system' | 'full'> = ['system', 'full'];
  const seen = new Map<string, { topInset: number; window: DesktopWindowInfo }>();

  for (const chrome of chromes) {
    const key = `${namespace}-${chrome}`;
    defer(() => closeKeyedSurface(app, key));

    const before = await desktop.windows();
    const opened = await openWindow(app, chrome, key);
    expect(opened.kind).toBe('page');
    expect(opened.realized).toBe('window');
    expect(opened.key).toBe(key);
    expect(opened.visible).toBeTruthy();
    expect(opened.alive).toBeTruthy();

    const page = await waitForSurfacePage(
      app,
      key,
      chrome === 'full' ? FULL_DRAG_STRIP_HEIGHT : 0,
    );
    expect(page.text).toContain('Surface Page');
    expect(page.topInset).toBe(chrome === 'full' ? FULL_DRAG_STRIP_HEIGHT : 0);
    // Presenting a window drives exactly one visibility transition, however
    // many times the platform and the opener report it.
    expect(page.showCount).toBe(1);

    const window = await eventually(
      () => desktop.windows(),
      (windows) => newSurfaceWindow(before, windows) !== undefined,
      {
        timeoutMs: 10_000,
        describe: `${chrome} chrome surface window on ${platform}`,
      },
    ).then((windows) => newSurfaceWindow(before, windows));
    if (!window) {
      throw new Error(`${chrome} chrome did not create a visible top-level window`);
    }
    seen.set(chrome, { topInset: page.topInset, window });

    await closeKeyedSurface(app, key);
    await eventually(
      () => app.eval({
        script: `return lx.surface.get(${JSON.stringify(key)}) == null`,
      }),
      (closed) => closed === true,
      { describe: `${chrome} chrome surface to close`, timeoutMs: 10_000 });
  }

  const system = seen.get('system');
  const full = seen.get('full');
  if (!system || !full) {
    throw new Error('expected both system and full chrome windows');
  }
  expect(system.topInset).toBe(0);
  expect(full.topInset).toBe(FULL_DRAG_STRIP_HEIGHT);
  // Both windows belong to this host process; a stranger's window matching the
  // size filter would not.
  expect(full.window.pid).toBe(system.window.pid);
  if (platform === 'windows') {
    // Full chrome has no system title bar, so the same requested content
    // size yields a shorter outer frame than system chrome.
    expect(full.window.bounds.h).toBeLessThan(system.window.bounds.h);
  }
});

windowTest('deliver a child page message to its opener before closing', {
  id: 'DESKTOP-SURFACE-MESSAGE-001',
  covers: ['PageSurface.onMessage', 'PageMessagePort.postMessage'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-MESSAGE-001');
  await desktopPlatform();
  const key = `${namespace}-message`;
  const stateKey = `__lingxiaSurfaceMessage_${namespace.replace(/-/g, '_')}`;
  const marker = `surface-message-${namespace}`;

  defer(() => closeKeyedSurface(app, key));
  defer(async () => {
    await app.eval({
      script: `
        const state = globalThis[${JSON.stringify(stateKey)}];
        if (state?.off) state.off();
        delete globalThis[${JSON.stringify(stateKey)}];
      `,
    }).catch(() => undefined);
  });

  const opened = await openWindow(app, 'system', key);
  expect(opened.kind).toBe('page');
  await waitForSurfacePage(app, key, 0);

  await app.eval({
    script: `
      const handle = lx.surface.get(${JSON.stringify(key)});
      if (!handle) throw new Error('message surface was not registered');
      const state = { messages: [], off: null };
      state.off = handle.onMessage((message) => state.messages.push(message));
      globalThis[${JSON.stringify(stateKey)}] = state;
    `,
  });

  await app.page.eval({
    page: 'surface',
    script: `(() => {
      const input = document.querySelector('input[placeholder="Message to parent page"]');
      if (!(input instanceof HTMLInputElement)) throw new Error('surface message input missing');
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
      if (!setter) throw new Error('HTMLInputElement value setter missing');
      setter.call(input, ${JSON.stringify(marker)});
      input.dispatchEvent(new InputEvent('input', {
        bubbles: true,
        data: ${JSON.stringify(marker)},
        inputType: 'insertText',
      }));
      return input.value;
    })()`,
  });
  await waitForElementAttribute(
    app,
    'surface',
    'input[placeholder="Message to parent page"]',
    'data-controlled-value',
    marker,
  );
  await app.page.click({
    page: 'surface',
    css: '[data-testid="surface-send-message"]',
  });

  const messages = await eventually(
    () => app.eval({
      script: `return globalThis[${JSON.stringify(stateKey)}]?.messages ?? []`,
    }) as Promise<Array<{ message?: string; timestamp?: number }>>,
    (value) => value.some((message) => message.message === marker),
    { describe: 'surface message delivered to opener', timeoutMs: 10_000 },
  );
  const received = messages.find((message) => message.message === marker);
  expect(typeof received?.timestamp).toBe('number');

  await eventually(
    () => app.eval({
      script: `return lx.surface.get(${JSON.stringify(key)}) == null`,
    }),
    (closed) => closed === true,
    { describe: 'messaging surface to close itself', timeoutMs: 10_000 },
  );
});

windowTest('push a message from the opener into its page window', {
  id: 'DESKTOP-SURFACE-POST-001',
  covers: ['PageSurface.postMessage', 'PageSurface.close', 'PageSurface.onClose', 'PageSurface.alive'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-POST-001');
  await desktopPlatform();
  const key = `${namespace}-post`;
  const stateKey = `__lingxiaSurfacePost_${namespace.replace(/-/g, '_')}`;
  defer(() => closeKeyedSurface(app, key));

  const opened = await openWindow(app, 'system', key);
  expect(opened.alive).toBeTruthy();
  await waitForSurfacePage(app, key, 0);

  await app.eval({
    script: `
      const handle = lx.surface.get(${JSON.stringify(key)});
      if (!handle) throw new Error('post surface was not registered');
      handle.postMessage({ ping: ${JSON.stringify(namespace)} });
    `,
  });
  const inbound = await eventually(
    () => app.page.eval({
      page: 'surface',
      script: `(() => {
        const text = document.querySelector('[data-testid="surface-inbound"]');
        const count = document.querySelector('[data-testid="surface-inbound-count"]');
        return { text: text ? text.textContent.trim() : '', count: count ? count.textContent.trim() : '' };
      })()`,
    }) as Promise<{ text: string; count: string }>,
    (value) => value.text.includes(namespace),
    { describe: 'opener message to reach the surface page', timeoutMs: 10_000 },
  );
  expect(JSON.parse(inbound.text)).toEqual({ ping: namespace });
  expect(inbound.count).toBe('1');

  // Closing from the opener flips `alive` on the same handle once the native
  // close lands — after close() itself resolves, so observe rather than read.
  await app.eval({
    timeoutMs: 15_000,
    script: `
      const handle = lx.surface.get(${JSON.stringify(key)});
      const state = { handle, closed: [] };
      handle.onClose((event) => state.closed.push(event));
      globalThis[${JSON.stringify(stateKey)}] = state;
      await handle.close();
    `,
  });
  defer(async () => {
    await app.eval({ script: `delete globalThis[${JSON.stringify(stateKey)}]` }).catch(() => undefined);
  });
  const closed = await eventually(
    () => app.eval({
      script: `
        const state = globalThis[${JSON.stringify(stateKey)}];
        return { alive: state.handle.alive, visible: state.handle.visible, closed: state.closed };
      `,
    }) as Promise<{ alive: boolean; visible: boolean; closed: Array<{ id?: string }> }>,
    (state) => !state.alive && !state.visible && state.closed.length >= 1,
    { describe: 'closed surface handle to report alive=false, visible=false, and fire onClose', timeoutMs: 10_000 },
  );
  expect(closed.closed.length).toBe(1);
  expect(closed.closed[0]?.id).toBe(opened.id);
});
