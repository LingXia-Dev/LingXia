import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';
import { expect, test, type Fixture, type JsonValue, type TestApp } from '@lingxia/test';
import type {
  TerminalPaneSnapshot,
  TerminalPaneTree,
  TerminalWorkspaceSnapshot,
} from '@lingxia/types/automation';

const SETTINGS_APP_ID = 'app.lingxia.terminal-settings';

const targetPlatform = globalThis.__LINGXIA_AUTOMATION_HOST__?.control?.platform?.toLocaleLowerCase();
test.configure({
  skip: !targetPlatform || targetPlatform === 'macos' || targetPlatform === 'windows'
    ? undefined
    : 'macOS and Windows only',
});

function leaves(tree: TerminalPaneTree | undefined): TerminalPaneSnapshot[] {
  if (!tree) return [];
  if (tree.kind === 'leaf') return [tree.pane];
  return tree.children.flatMap(leaves);
}

function activeTree(snapshot: TerminalWorkspaceSnapshot): TerminalPaneTree | undefined {
  return snapshot.tabs.find((tab) => tab.active)?.tree;
}

async function waitFor<T>(operation: () => Promise<T | undefined>, label: string): Promise<T> {
  const deadline = Date.now() + 8_000;
  while (Date.now() < deadline) {
    const value = await operation();
    if (value !== undefined) return value;
    await new Promise<void>((resolve) => setTimeout(() => resolve(), 25));
  }
  throw new Error(`${label} was not observed`);
}

/** The runtime id of the terminal declared under `key`, read from the host layout. */
async function terminalSurfaceId(app: TestApp, key: string): Promise<string> {
  return await waitFor(async () => {
    const { identities } = await app.surfaceLayout();
    return Object.entries(identities)
      .find(([, owner]) => owner.declaration === 'terminal' && owner.key === key)?.[0];
  }, `terminal surface ${key} in the layout`);
}

/** Close the surface handles Logic keeps under `key`, and forget them. */
async function closeHandles(app: TestApp, key: string): Promise<void> {
  await app.logic.eval({ timeout: 20_000 }, async (_, key) => {
    const held = globalThis as unknown as Record<string, Record<string, { alive: boolean; close(): Promise<void> } | undefined> | undefined>;
    const handles = held[key];
    delete held[key];
    for (const handle of Object.values(handles ?? {})) {
      if (handle?.alive) await handle.close();
    }
  }, key);
}

test('publishes and mutates the native nested pane tree without deferred layout', {
  id: 'DESKTOP-TERMINAL-001',
  timeout: 90_000,
  covers: ['lx.surface.openDeclared', 'lx.host.terminal'],
}, async (t) => {
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  const token = `automation-terminal-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  await app.logic.eval({ timeout: 20_000 }, async ({ lx }, token) => {
    const handle = await lx.surface.openDeclared('terminal', { key: token, as: 'main' });
    (globalThis as unknown as Record<string, unknown>).__terminalAutomationHandles = { terminal: handle };
  }, token);
  const surfaceId = await terminalSurfaceId(app, token);

  const terminal = t.automation.terminal;
  try {
    const initial = await terminal.snapshot({ surface: surfaceId });
    expect(initial.presentation).toBe('main');
    expect(initial.paneCount).toBe(1);

    const splitRight = await terminal.split({ surface: surfaceId, direction: 'right' });
    const rightTree = activeTree(splitRight);
    expect(rightTree?.kind).toBe('split');
    if (!rightTree || rightTree.kind !== 'split') throw new Error('right split tree is missing');
    expect(rightTree.axis).toBe('horizontal');
    expect(rightTree.children.length).toBe(2);
    expect(leaves(rightTree).length).toBe(2);

    const splitDown = await terminal.split({ surface: surfaceId, direction: 'down' });
    const nested = activeTree(splitDown);
    expect(nested?.kind).toBe('split');
    if (!nested || nested.kind !== 'split') throw new Error('nested split tree is missing');
    expect(nested.axis).toBe('horizontal');
    expect(nested.children.length).toBe(2);
    const right = nested.children[1];
    expect(right.kind).toBe('split');
    if (right.kind !== 'split') throw new Error('right column did not split');
    expect(right.axis).toBe('vertical');
    expect(right.children.length).toBe(2);

    const panes = leaves(nested);
    expect(panes.length).toBe(3);
    expect(panes.filter((pane) => pane.active).length).toBe(1);
    for (const pane of panes) {
      expect(pane.visible).toBeTruthy();
      expect(pane.frame.width).toBeGreaterThan(20);
      expect(pane.frame.height).toBeGreaterThan(20);
      expect(pane.grid.cols).toBeGreaterThan(0);
      expect(pane.grid.rows).toBeGreaterThan(0);
    }
  } finally {
    await closeHandles(app, '__terminalAutomationHandles');
  }
});

test('keeps a maximized terminal maximized when a tab opens', {
  id: 'DESKTOP-TERMINAL-002',
  timeout: 90_000,
  covers: ['lx.surface.openDeclared'],
}, async (t) => {
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  const token = `automation-terminal-tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  // An aside is the shape that can be maximized: `main` already fills the
  // content area, so it could not show the state being clobbered.
  await app.logic.eval({ timeout: 20_000 }, async ({ lx }, token) => {
    const handle = await lx.surface.openDeclared('terminal', { key: token, as: 'aside', edge: 'bottom' });
    (globalThis as unknown as Record<string, unknown>).__terminalTabAutomationHandles = { terminal: handle };
  }, token);
  const surfaceId = await terminalSurfaceId(app, token);

  const terminal = t.automation.terminal;
  try {
    const docked = await terminal.snapshot({ surface: surfaceId });
    expect(docked.maximized).toBe(false);

    const maximized = await terminal.setMaximized({ surface: surfaceId, maximized: true });
    expect(maximized.maximized).toBe(true);
    const tabsBefore = maximized.tabCount;

    // Opening a tab renames the active tab, which syncs the shell layout and
    // used to re-present the panel in its docked state.
    const afterNewTab = await terminal.newTab({ surface: surfaceId });
    expect(afterNewTab.tabCount).toBe(tabsBefore + 1);
    expect(afterNewTab.maximized).toBe(true);

    const settled = await terminal.snapshot({ surface: surfaceId });
    expect(settled.maximized).toBe(true);
  } finally {
    await closeHandles(app, '__terminalTabAutomationHandles');
  }
});

test('applies a chosen color scheme to native chrome at once', {
  id: 'DESKTOP-TERMINAL-003',
  timeout: 90_000,
  covers: ['lx.surface.openDeclared', 'lx.surface.openApp'],
}, async (t) => {
  const app = t.automation.lxapp(SHOWCASE_APP_ID);
  const token = `automation-terminal-theme-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  // One surface per open. Opening both at once reports only that it timed
  // out, which says nothing about which surface never settled — and a native
  // surface and a bundled lxapp settle along different paths.
  await app.logic.eval({ timeout: 20_000 }, async ({ lx }, token) => {
    const terminal = await lx.surface.openDeclared('terminal', { key: token, as: 'main' });
    (globalThis as unknown as Record<string, unknown>).__terminalThemeAutomationHandles = { terminal };
  }, token);
  const terminalId = await terminalSurfaceId(app, token);
  // Opening a bundled lxapp aside has to reach the host and come back, so it
  // is slower than the native surface above and pays a cold start on CI.
  await app.logic.eval({ timeout: 60_000 }, async ({ lx }, settingsAppId) => {
    const settings = await lx.surface.openApp(settingsAppId, { as: 'aside', edge: 'right' });
    const held = globalThis as unknown as Record<string, Record<string, unknown>>;
    held.__terminalThemeAutomationHandles.settings = settings;
  }, SETTINGS_APP_ID);
  const refs = { terminal: terminalId };
  const terminal = t.automation.terminal;
  const settingsApp = t.automation.lxapp(SETTINGS_APP_ID);
  let initial: TerminalWorkspaceSnapshot | undefined;
  const previousAppearance = await app.logic.eval(({ lx }) => lx.host.appearancePreference.get() ?? null);

  try {
    await expect(settingsApp.view.testId('schemes')).toBeVisible({ timeout: 10_000 });
    const runtime = await settingsApp.logic.eval(async ({ lx }) => ({
      terminal: typeof lx.host.terminal.settings.get,
      fileSystem: typeof lx.fs,
    }));
    expect(runtime.terminal).toBe('function');
    expect(runtime.fileSystem).toBe('undefined');

    // The page lists only the schemes for the product's light or dark; light ships one built-in,
    // so pin dark when the current slot cannot offer a second card.
    const installed = await settingsApp.logic.eval(async ({ lx }) => (await lx.host.terminal.colorSchemes.list())
      .map((entry) => ({
        name: entry.name,
        background: entry.scheme.background.toLowerCase(),
        cursor: (entry.scheme.cursorColor ?? entry.scheme.foreground).toLowerCase(),
      })));
    const shown = async () => {
      const cards = [];
      for (const entry of installed) {
        if (await settingsApp.view.testId(`scheme-${entry.name}`).count() > 0) cards.push(entry);
      }
      return cards;
    };
    let cards = await shown();
    if (cards.length < 2) {
      await app.logic.eval(async ({ lx }) => {
        await lx.host.appearancePreference.set('dark');
        return true;
      });
      cards = await waitFor(async () => {
        const next = await shown();
        return next.length >= 2 ? next : undefined;
      }, 'dark-slot color schemes');
    }

    initial = await terminal.snapshot({ surface: refs.terminal });
    // Identical colors do not repaint, so choose a visibly different background.
    const target = cards.find((card) => card.background !== initial!.chrome.surface);
    if (!target) throw new Error('terminal settings did not show a visually distinct color scheme');
    const slot = await settingsApp.logic.eval(async ({ lx }) => (await lx.host.terminal.settings.get()).effective.appearance);

    await settingsApp.view.testId(`scheme-${target.name}`).click();
    const applied = await waitFor(async () => {
      const snapshot = await terminal.snapshot({ surface: refs.terminal });
      const theme = snapshot.config.theme as { light?: string; dark?: string } | undefined;
      return snapshot.configGeneration > initial!.configGeneration
        && theme?.[slot] === target.name
        && snapshot.chrome.surface === target.background
        && snapshot.chrome.cursor === target.cursor
        ? snapshot
        : undefined;
    }, 'persisted terminal color scheme');
    expect(applied.chrome.surface).toBe(target.background);
    await expect(settingsApp.view.testId(`scheme-${target.name}`)).toBeChecked();
  } finally {
    await app.logic.eval(async ({ lx }, previous) => {
      if (previous !== null) await lx.host.appearancePreference.set(previous);
      return true;
    }, previousAppearance).catch(() => undefined);
    if (initial) {
      // Restores the workspace snapshot's config as the settings patch it was.
      await settingsApp.logic.eval({ timeout: 20_000 }, async ({ lx }, config) => {
        const settings = lx.host.terminal.settings;
        const current = await settings.get();
        await settings.update({ ...(config as object), ifRevision: current.revision });
      }, initial.config as unknown as JsonValue);
    }
    await closeHandles(app, '__terminalThemeAutomationHandles');
  }
});
