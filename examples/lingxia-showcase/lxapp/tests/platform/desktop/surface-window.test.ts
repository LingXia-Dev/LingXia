import { expect, test } from '@rongjs/test';
import type { DesktopWindowInfo } from 'lingxia-types/automation';
import { showcaseApp } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { contract, eventually } from '../../support/contract.js';

const testArgs = test.args as Record<string, string>;
const targetPlatform = testArgs.platform?.toLocaleLowerCase();
const selectedGate = testArgs.gate?.toLocaleLowerCase();

const FULL_DRAG_STRIP_HEIGHT = 28;

interface OpenedWindow {
  id: string;
  kind: string;
  realized: string;
  visible: boolean;
  alive: boolean;
  chrome: string;
}

interface SurfacePageSnapshot {
  text: string;
  topInset: number;
  query: string;
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
        const root = document.querySelector('[data-testid="surface-page"]');
        const layout = window.lxPageChrome && window.lxPageChrome.layout;
        const show = document.querySelector('[data-testid="surface-page"] .font-mono');
        return {
          text: document.body ? document.body.innerText : '',
          topInset: layout ? layout.topInset : -1,
          query: root ? (root.innerText || '') : '',
          showCount: Number((show && show.textContent) || 0),
        };
      })()`,
    }) as Promise<SurfacePageSnapshot>,
    (snapshot) => (
      typeof snapshot?.text === 'string'
      && snapshot.text.includes('Surface Page')
      && snapshot.text.includes(fixture)
      && snapshot.topInset === expectedTopInset
    ),
    {
      timeoutMs: 10_000,
      describe: `surface window page ${fixture} with topInset ${expectedTopInset}`,
    },
  );
}

function newSurfaceWindow(
  before: DesktopWindowInfo[],
  after: DesktopWindowInfo[],
): DesktopWindowInfo | undefined {
  const known = new Set(before.map((window) => window.id));
  return after
    .filter((window) => (
      window.visible
      && !window.minimized
      && !known.has(window.id)
      && window.process.toLocaleLowerCase() !== 'msedgewebview2'
      && window.bounds.w >= 400
      && window.bounds.h >= 400
    ))
    .sort((left, right) => (
      Math.abs(left.bounds.w - 480) + Math.abs(left.bounds.h - 640)
      - (Math.abs(right.bounds.w - 480) + Math.abs(right.bounds.h - 640))
    ))[0];
}

contract({
  id: 'DESKTOP-SURFACE-WINDOW-001',
  title: 'open a page window with system chrome and with full chrome',
  covers: [
    'lx.surface.openPage',
    'lx.surface.get',
    'lx.supports',
  ],
  layer: 'logic',
  levels: ['semantic', 'lifecycle', 'boundary'],
  scope: 'desktop',
  expectedOutcome: 'supported',
}, async ({ app, namespace, defer }) => {
  // Gated desktop jobs already cover layout; this case needs a free host window.
  if (selectedGate) return;

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
    expect(opened.visible).toBeTruthy();
    expect(opened.alive).toBeTruthy();

    const page = await waitForSurfacePage(
      app,
      key,
      chrome === 'full' ? FULL_DRAG_STRIP_HEIGHT : 0,
    );
    expect(page.text).toContain('Surface Page');
    expect(page.topInset).toBe(chrome === 'full' ? FULL_DRAG_STRIP_HEIGHT : 0);

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
      { describe: `${chrome} chrome surface to close`, timeoutMs: 10_000 },
    );
  }

  const system = seen.get('system');
  const full = seen.get('full');
  if (!system || !full) {
    throw new Error('expected both system and full chrome windows');
  }
  expect(system.topInset).toBe(0);
  expect(full.topInset).toBe(FULL_DRAG_STRIP_HEIGHT);
  if (platform === 'windows') {
    // Full chrome has no system title bar, so the same requested content
    // size yields a shorter outer frame than system chrome.
    expect(full.window.bounds.h < system.window.bounds.h).toBeTruthy();
  }
});
