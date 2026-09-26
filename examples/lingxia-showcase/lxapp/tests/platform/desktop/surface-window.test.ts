import { expect, spec, type Fixture, type TestApp } from '@lingxia/test';
import type { DesktopAxNode, DesktopWindowInfo } from '@lingxia/types/automation';
import { runtimePlatform } from '../../helpers/platform.js';
import { waitForElementAttribute } from '../../helpers/page.js';
import { bindFixture, eventually } from '../../helpers/poll.js';
import { SHOWCASE_APP_ID, rawApp } from '../../helpers/app.js';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

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

async function desktopPlatform(t: Fixture): Promise<string> {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  const actual = await runtimePlatform(raw);
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

async function closeKeyedSurface(app: TestApp, key: string): Promise<void> {
  await raw.eval({
    timeoutMs: 15_000,
    script: `
      const handle = lx.surface.getByKey(${JSON.stringify(key)});
      if (handle) await handle.close();
    `,
  });
}

async function openWindow(
  app: TestApp,
  chrome: 'system' | 'full',
  key: string,
): Promise<OpenedWindow> {
  return await raw.eval({
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
  app: TestApp,
  fixture: string,
  expectedTopInset: number,
): Promise<SurfacePageSnapshot> {
  await raw.page.waitFor({
    page: 'surface',
    css: '[data-testid="surface-page"]',
    state: 'visible',
    timeoutMs: 15_000,
  });
  return eventually(
    () => raw.page.eval({
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

windowTest('native close disposes a secondary window in the dock and tray host', {
  id: 'DESKTOP-SURFACE-NATIVE-CLOSE-001',
  covers: ['PageSurface.onClose', 'PageSurface.alive', 'PageSurface.visible'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-NATIVE-CLOSE-001');
  await desktopPlatform(t);
  const desktop = t.automation.desktop;
  const key = `${namespace}-native-close`;
  const stateKey = `__surfaceNativeClose_${namespace.replace(/-/g, '_')}`;
  defer(() => closeKeyedSurface(app, key));
  defer(async () => {
    await raw.eval({ script: `delete globalThis[${JSON.stringify(stateKey)}];` });
  });
  const before = await desktop.windows();
  await openWindow(app, 'system', key);
  await waitForSurfacePage(app, key, 0);
  const window = await eventually(
    async () => newSurfaceWindow(before, await desktop.windows()),
    (window) => !!window,
    { describe: 'secondary native window to appear', timeoutMs: 10_000 },
  );
  if (!window) throw new Error('secondary native window was not found');
  await raw.eval({
    script: `
      const handle = lx.surface.getByKey(${JSON.stringify(key)});
      const state = { handle, closed: 0 };
      handle.onClose(() => state.closed++);
      globalThis[${JSON.stringify(stateKey)}] = state;
    `,
  });
  // Showcase declares a nonexclusive tray. Native close must reach the surface
  // close handler, rather than merely hiding its HWND because a tray exists.
  await desktop.window.close({ window: window.id });
  const state = await eventually(
    () => raw.eval({
      script: `
        const state = globalThis[${JSON.stringify(stateKey)}];
        return { alive: state.handle.alive, visible: state.handle.visible, closed: state.closed };
      `,
    }) as Promise<{ alive: boolean; visible: boolean; closed: number }>,
    (state) => !state.alive && !state.visible && state.closed > 0,
    { describe: 'native close to dispose the surface and notify its owner', timeoutMs: 10_000 },
  );
  expect(state.closed).toBe(1);
});

windowTest('open a page window with system chrome and with full chrome', {
  id: 'DESKTOP-SURFACE-WINDOW-001',
  covers: [
    'lx.surface.openPage',
    'lx.surface.getByKey',
    'lx.supports',
    'PageSurface.kind',
    'PageSurface.realized',
    'PageSurface.id',
    'PageSurface.key',
    'PageSurface.alive',
    'PageSurface.visible',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 90_000,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-WINDOW-001');
  const platform = await desktopPlatform(t);
  const desktop = t.automation.desktop;
  const fullOffered = await raw.eval({
    script: `return !!lx.supports('surface.window.fullChrome')`,
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
      () => raw.eval({
        script: `return lx.surface.getByKey(${JSON.stringify(key)}) == null`,
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

function captionButtonNames(
  platform: string,
  kind: 'minimize' | 'maximize' | 'close',
  maximized = false,
): string[] {
  if (platform === 'macos') {
    if (kind === 'close') return ['close'];
    if (kind === 'minimize') return ['minimize'];
    return ['zoom', 'full screen'];
  }
  if (kind === 'close') return ['Close'];
  if (kind === 'minimize') return ['Minimize'];
  return maximized ? ['Restore'] : ['Maximize'];
}

function nameMatchesKeyword(name: string, keyword: string): boolean {
  return name.toLocaleLowerCase().includes(keyword.toLocaleLowerCase());
}

function inWindowCaption(window: DesktopWindowInfo, node: DesktopAxNode): boolean {
  // macOS traffic lights sit a few points past the reported frame.
  const slop = 12;
  const top = window.bounds.y - slop;
  const bottom = window.bounds.y + Math.min(48, Math.max(28, Math.round(window.bounds.h * 0.12))) + slop;
  return node.role === 'button'
    && node.enabled
    && node.rect.w > 0
    && node.rect.h > 0
    && node.rect.y + node.rect.h > top
    && node.rect.y < bottom
    && node.rect.x >= window.bounds.x - slop
    && node.rect.x + node.rect.w <= window.bounds.x + window.bounds.w + slop;
}

async function captionButton(
  desktop: ReturnType<typeof lx.automation>['desktop'],
  window: DesktopWindowInfo,
  names: string[],
): Promise<DesktopAxNode> {
  let observation = `no node matching ${names.join('|')}`;
  const node = await eventually(
    async () => {
      const current = await desktop.window.status({ window: window.id });
      for (const name of names) {
        const nodes = await desktop.ax.query({
          window: current.id,
          match: `name:${name}`,
          all: true,
        }).catch(() => [] as DesktopAxNode[]);
        const match = nodes.find((candidate) => (
          nameMatchesKeyword(candidate.name, name) && inWindowCaption(current, candidate)
        ));
        if (match) return match;
        observation = `${name}: ${nodes.map((node) => `${node.role}/${node.name}`).join(', ') || 'none'}`;
      }
      return undefined;
    },
    (node): node is DesktopAxNode => node !== undefined,
    { timeoutMs: 10_000, describe: `caption ${names.join('/')}` },
  );
  if (!node) throw new Error(`caption ${names.join('/')} was not found; last AX: ${observation}`);
  return node;
}

async function invokeCaption(
  desktop: ReturnType<typeof lx.automation>['desktop'],
  window: DesktopWindowInfo,
  names: string[],
): Promise<void> {
  const node = await captionButton(desktop, window, names);
  await desktop.ax.invoke({ window: window.id, match: `id:${node.id}` });
}

windowTest('caption buttons stay on top and can close both chrome modes', {
  id: 'DESKTOP-SURFACE-WINDOW-CHROME-001',
  covers: [
    'lx.surface.openPage',
    'PageSurface.alive',
    'PageSurface.visible',
    'PageSurface.onClose',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 120_000,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-WINDOW-CHROME-001');
  const platform = await desktopPlatform(t);
  const desktop = t.automation.desktop;
  const chromes: Array<'system' | 'full'> = ['system', 'full'];

  for (const chrome of chromes) {
    const key = `${namespace}-${chrome}`;
    defer(() => closeKeyedSurface(app, key));
    defer(async () => {
      await raw.eval({
        script: `delete globalThis[${JSON.stringify(`__surfaceCaption_${namespace}_${chrome}`)}];`,
      });
    });
    const before = await desktop.windows();
    await openWindow(app, chrome, key);
    await waitForSurfacePage(app, key, chrome === 'full' ? FULL_DRAG_STRIP_HEIGHT : 0);
    const opened = await eventually(
      () => desktop.windows(),
      (windows) => newSurfaceWindow(before, windows) !== undefined,
      { timeoutMs: 10_000, describe: `${chrome} chrome window for caption checks` },
    ).then((windows) => newSurfaceWindow(before, windows));
    if (!opened) throw new Error(`${chrome} chrome window was not found`);

    await desktop.window.activate({ window: opened.id });
    const current = await desktop.window.status({ window: opened.id });
    expect((await captionButton(
      desktop,
      current,
      captionButtonNames(platform, 'close'),
    )).name.trim().length).toBeGreaterThan(0);

    if (platform === 'windows') {
      await invokeCaption(desktop, current, captionButtonNames(platform, 'maximize', false));
      await eventually(
        () => desktop.window.status({ window: opened.id }),
        (status) => status.maximized,
        { timeoutMs: 8_000, describe: `${chrome} caption maximize` },
      );
      await invokeCaption(
        desktop,
        await desktop.window.status({ window: opened.id }),
        captionButtonNames(platform, 'maximize', true),
      );
      await eventually(
        () => desktop.window.status({ window: opened.id }),
        (status) => !status.maximized,
        { timeoutMs: 8_000, describe: `${chrome} caption restore` },
      );
      await invokeCaption(
        desktop,
        await desktop.window.status({ window: opened.id }),
        captionButtonNames(platform, 'minimize'),
      );
      await eventually(
        () => desktop.window.status({ window: opened.id }),
        (status) => status.minimized,
        { timeoutMs: 8_000, describe: `${chrome} caption minimize` },
      );
      await desktop.window.activate({ window: opened.id });
      await eventually(
        () => desktop.window.status({ window: opened.id }),
        (status) => !status.minimized && status.visible,
        { timeoutMs: 8_000, describe: `${chrome} window restored after minimize` },
      );
    }

    await raw.eval({
      script: `
        const handle = lx.surface.getByKey(${JSON.stringify(key)});
        const state = { handle, closed: 0 };
        handle.onClose(() => state.closed++);
        globalThis[${JSON.stringify(`__surfaceCaption_${namespace}_${chrome}`)}] = state;
      `,
    });
    const closable = await desktop.window.status({ window: opened.id });
    await desktop.window.activate({ window: closable.id });
    await invokeCaption(desktop, closable, captionButtonNames(platform, 'close'));
    const state = await eventually(
      () => raw.eval({
        script: `
          const state = globalThis[${JSON.stringify(`__surfaceCaption_${namespace}_${chrome}`)}];
          return { alive: state.handle.alive, visible: state.handle.visible, closed: state.closed };
        `,
      }) as Promise<{ alive: boolean; visible: boolean; closed: number }>,
      (state) => !state.alive && !state.visible && state.closed > 0,
      { timeoutMs: 10_000, describe: `${chrome} chrome caption close to dispose the surface` },
    );
    expect(state.closed).toBe(1);
    await raw.eval({
      script: `delete globalThis[${JSON.stringify(`__surfaceCaption_${namespace}_${chrome}`)}];`,
    }).catch(() => undefined);
  }
});

windowTest('deliver a child page message to its opener before closing', {
  id: 'DESKTOP-SURFACE-MESSAGE-001',
  covers: ['PageSurface.onMessage', 'PageMessagePort.postMessage'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-MESSAGE-001');
  await desktopPlatform(t);
  const key = `${namespace}-message`;
  const stateKey = `__lingxiaSurfaceMessage_${namespace.replace(/-/g, '_')}`;
  const marker = `surface-message-${namespace}`;

  defer(() => closeKeyedSurface(app, key));
  defer(async () => {
    await raw.eval({
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

  await raw.eval({
    script: `
      const handle = lx.surface.getByKey(${JSON.stringify(key)});
      if (!handle) throw new Error('message surface was not registered');
      const state = { messages: [], off: null };
      state.off = handle.onMessage((message) => state.messages.push(message));
      globalThis[${JSON.stringify(stateKey)}] = state;
    `,
  });

  await raw.page.eval({
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
    raw,
    'surface',
    'input[placeholder="Message to parent page"]',
    'data-controlled-value',
    marker,
  );
  await app.view.testId("surface-send-message", { page: 'surface' }).click();

  const messages = await eventually(
    () => raw.eval({
      script: `return globalThis[${JSON.stringify(stateKey)}]?.messages ?? []`,
    }) as Promise<Array<{ message?: string; timestamp?: number }>>,
    (value) => value.some((message) => message.message === marker),
    { describe: 'surface message delivered to opener', timeoutMs: 10_000 },
  );
  const received = messages.find((message) => message.message === marker);
  expect(typeof received?.timestamp).toBe('number');

  await eventually(
    () => raw.eval({
      script: `return lx.surface.getByKey(${JSON.stringify(key)}) == null`,
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
  await desktopPlatform(t);
  const key = `${namespace}-post`;
  const stateKey = `__lingxiaSurfacePost_${namespace.replace(/-/g, '_')}`;
  defer(() => closeKeyedSurface(app, key));

  const opened = await openWindow(app, 'system', key);
  expect(opened.alive).toBeTruthy();
  await waitForSurfacePage(app, key, 0);

  await raw.eval({
    script: `
      const handle = lx.surface.getByKey(${JSON.stringify(key)});
      if (!handle) throw new Error('post surface was not registered');
      handle.postMessage({ ping: ${JSON.stringify(namespace)} });
    `,
  });
  const inbound = await eventually(
    () => raw.page.eval({
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
  await raw.eval({
    timeoutMs: 15_000,
    script: `
      const handle = lx.surface.getByKey(${JSON.stringify(key)});
      const state = { handle, closed: [] };
      handle.onClose((event) => state.closed.push(event));
      globalThis[${JSON.stringify(stateKey)}] = state;
      await handle.close();
    `,
  });
  defer(async () => {
    await raw.eval({ script: `delete globalThis[${JSON.stringify(stateKey)}]` }).catch(() => undefined);
  });
  const closed = await eventually(
    () => raw.eval({
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
