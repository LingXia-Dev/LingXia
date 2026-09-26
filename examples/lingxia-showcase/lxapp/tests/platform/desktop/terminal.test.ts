import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { expect, spec, type Fixture, type JsonValue, type TestApp } from '@lingxia/test';
import type {
  TerminalPaneSnapshot,
  TerminalPaneTree,
  TerminalWorkspaceSnapshot,
} from '@lingxia/types/automation';
import type { ProbeDocument, ProbeElement } from '../../helpers/view.js';

const SETTINGS_APP_ID = 'app.lingxia.terminal-settings';

const targetPlatform = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>).platform?.toLocaleLowerCase();
const desktopTerminalTest =
  !targetPlatform || targetPlatform === 'macos' || targetPlatform === 'windows'
    ? spec
    : spec.skip;

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

async function waitForSave(settings: TestApp, enabled: boolean): Promise<void> {
  await waitFor(async () => {
    const save = await settings.view.css('#save').first().query();
    return save.exists && save.enabled === enabled ? true : undefined;
  }, enabled ? 'dirty Terminal Settings' : 'applied Terminal Settings');
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

desktopTerminalTest('publishes and mutates the native nested pane tree without deferred layout', {
  id: 'DESKTOP-TERMINAL-001',
  timeout: 90_000,
  covers: ['lx.shell.openDeclared', 'lx.terminal'],
}, async (t) => {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  const token = `automation-terminal-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const surfaceId = await app.logic.eval({ timeout: 20_000 }, async ({ lx }, token) => {
    const handle = await lx.shell.openDeclared('terminal', { key: token, as: 'main' });
    (globalThis as unknown as Record<string, unknown>).__terminalAutomationHandles = { terminal: handle };
    return handle.id;
  }, token);

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

desktopTerminalTest('keeps a maximized terminal maximized when a tab opens', {
  id: 'DESKTOP-TERMINAL-002',
  timeout: 90_000,
  covers: ['lx.shell.openDeclared'],
}, async (t) => {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  const token = `automation-terminal-tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  // An aside is the shape that can be maximized: `main` already fills the
  // content area, so it could not show the state being clobbered.
  const surfaceId = await app.logic.eval({ timeout: 20_000 }, async ({ lx }, token) => {
    const handle = await lx.shell.openDeclared('terminal', { key: token, as: 'aside', edge: 'bottom' });
    (globalThis as unknown as Record<string, unknown>).__terminalTabAutomationHandles = { terminal: handle };
    return handle.id;
  }, token);

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

desktopTerminalTest('applies a selected color scheme to native chrome before Apply', {
  id: 'DESKTOP-TERMINAL-003',
  timeout: 90_000,
  covers: ['lx.shell.openDeclared', 'lx.shell.openApp'],
}, async (t) => {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  const token = `automation-terminal-theme-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  // One surface per open. Opening both at once reports only that it timed
  // out, which says nothing about which surface never settled — and a native
  // surface and a bundled lxapp settle along different paths.
  const terminalId = await app.logic.eval({ timeout: 20_000 }, async ({ lx }, token) => {
    const terminal = await lx.shell.openDeclared('terminal', { key: token, as: 'main' });
    (globalThis as unknown as Record<string, unknown>).__terminalThemeAutomationHandles = { terminal };
    return terminal.id;
  }, token);
  // Opening a bundled lxapp aside has to reach the host and come back, so it
  // is slower than the native surface above and pays a cold start on CI.
  const settingsId = await app.logic.eval({ timeout: 60_000 }, async ({ lx }, settingsAppId) => {
    const settings = await lx.shell.openApp(settingsAppId, { as: 'aside', edge: 'right' });
    const held = globalThis as unknown as Record<string, Record<string, unknown>>;
    held.__terminalThemeAutomationHandles.settings = settings;
    return settings.id;
  }, SETTINGS_APP_ID);
  const refs = { terminal: terminalId, settings: settingsId };
  const terminal = t.automation.terminal;
  const settingsApp = t.apps.lxapp(SETTINGS_APP_ID);
  let initial: TerminalWorkspaceSnapshot | undefined;
  const previousAppearance = await app.logic.eval(({ lx }) => lx.host.control?.appearance.getPreference() ?? null);

  try {
    // The action bar is intentionally hidden until the draft becomes dirty;
    // its button is still a reliable page-readiness marker once attached.
    await settingsApp.view.css('#save').first().waitFor({ state: 'attached', timeout: 10_000 });
    await settingsApp.view.css('button[data-theme][aria-pressed="true"]').first()
      .waitFor({ state: 'visible', timeout: 10_000 });
    await waitForSave(settingsApp, false);
    const runtime = await settingsApp.logic.eval(async ({ lx }) => {
      const settings = await lx.terminal!.settings.get();
      return {
        terminal: typeof lx.terminal?.settings?.get,
        fileSystem: typeof lx.fs,
        appearance: settings.effective.appearance,
      };
    });
    expect(runtime.terminal).toBe('function');
    expect(runtime.fileSystem).toBe('undefined');

    const themeCards = async (): Promise<Array<{ name: string; pressed: boolean }>> =>
      settingsApp.view.eval(({ document }) => Array.from(document.querySelectorAll('button[data-theme]')).map((el) => ({
        name: el.getAttribute('data-theme') ?? '',
        pressed: el.getAttribute('aria-pressed') === 'true',
      })));

    // The product owns light/dark; this screen only lists schemes for the
    // active slot. Light ships one built-in, dark ships several, so pin dark
    // when the current slot cannot offer a second card to click.
    let cards = await themeCards();
    if (cards.length < 2) {
      await app.logic.eval(async ({ lx }) => {
        await lx.host.control!.appearance.setPreference('dark');
        return true;
      });
      cards = await waitFor(async () => {
        const next = await themeCards();
        return next.length >= 2 ? next : undefined;
      }, 'dark-slot color schemes');
    }

    initial = await terminal.snapshot({ surface: refs.terminal });
    // Imported aliases can have the same palette as an unselected built-in.
    // Choose a visibly different background: identical colors do not repaint.
    const target = await settingsApp.view.eval(({ document }, surface) => {
      const current = (document as unknown as ProbeDocument).createElement('span');
      current.style.backgroundColor = surface;
      const card = (Array.from(document.querySelectorAll('button[data-theme]')) as ProbeElement[]).find((el) =>
        el.getAttribute('aria-pressed') !== 'true'
        && el.style.backgroundColor !== current.style.backgroundColor);
      return card?.dataset.theme ?? null;
    }, initial.chrome.surface);
    if (!target) throw new Error('terminal settings did not publish a visually distinct color scheme');
    const slot = await settingsApp.logic.eval(async ({ lx }) => (await lx.terminal!.settings.get()).effective.appearance);

    // Click through the settings document: an OS pointer click can miss the
    // aside WebView even when the card is in the page DOM.
    const clicked = await settingsApp.view.eval(({ document }, css) => {
      const el = document.querySelector(css) as ProbeElement | null;
      if (!el) return { ok: false, reason: 'missing' };
      el.scrollIntoView({ block: 'center', inline: 'nearest' });
      el.click();
      return {
        ok: true,
        saveDisabled: document.getElementById('save')?.disabled === true,
      };
    }, `button[data-theme="${target}"]`);
    if (!clicked.ok) {
      throw new Error(`color scheme ${target} was not in the settings document`);
    }
    await waitForSave(settingsApp, true);

    const previewed = await waitFor(async () => {
      const snapshot = await terminal.snapshot({ surface: refs.terminal });
      return snapshot.visualGeneration !== initial!.visualGeneration
        ? snapshot
        : undefined;
    }, 'native terminal preview chrome');
    expect(previewed.configGeneration).toBe(initial.configGeneration);

    await settingsApp.view.eval(({ document }) => {
      const save = document.getElementById('save') as ProbeElement | null;
      if (!save) throw new Error('Apply is missing');
      save.click();
      return true;
    });
    await waitForSave(settingsApp, false);
    const applied = await waitFor(async () => {
      const snapshot = await terminal.snapshot({ surface: refs.terminal });
      const theme = snapshot.config.theme as { light?: string; dark?: string } | undefined;
      return snapshot.configGeneration > initial!.configGeneration
        && theme?.[slot] === target
        ? snapshot
        : undefined;
    }, 'persisted terminal color scheme');
    expect(applied.chrome.surface).toBe(previewed.chrome.surface);
    expect(applied.chrome.cursor).toBe(previewed.chrome.cursor);
  } finally {
    await app.logic.eval(async ({ lx }, previous) => {
      if (previous !== null) await lx.host.control?.appearance.setPreference(previous);
      return true;
    }, previousAppearance).catch(() => undefined);
    if (initial) {
      // Restores the workspace snapshot's config as the settings patch it was.
      await settingsApp.logic.eval({ timeout: 20_000 }, async ({ lx }, config) => {
        const settings = lx.terminal!.settings;
        const current = await settings.get();
        await settings.update(config as Parameters<typeof settings.update>[0], { ifRevision: current.revision });
      }, initial.config as unknown as JsonValue);
    }
    await closeHandles(app, '__terminalThemeAutomationHandles');
  }
});
