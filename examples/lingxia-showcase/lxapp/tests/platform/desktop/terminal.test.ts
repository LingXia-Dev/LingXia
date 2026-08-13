import { expect, test } from '@rongjs/test';
import type {
  PageDriver,
  TerminalPaneSnapshot,
  TerminalPaneTree,
  TerminalWorkspaceSnapshot,
} from 'lingxia-types/automation';
import { showcaseApp } from '../../helpers/app.js';

const targetPlatform = (test.args as Record<string, string>).platform?.toLocaleLowerCase();
const desktopTerminalTest =
  !targetPlatform || targetPlatform === 'macos' || targetPlatform === 'windows'
    ? test
    : test.skip;

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

async function waitForSave(page: PageDriver, enabled: boolean): Promise<void> {
  await waitFor(async () => {
    const save = await page.query({ css: '#save' });
    return save.exists && save.enabled === enabled ? true : undefined;
  }, enabled ? 'dirty Terminal Settings' : 'applied Terminal Settings');
}

desktopTerminalTest('publishes and mutates the native nested pane tree without deferred layout', async () => {
  const app = showcaseApp();
  const token = `automation-terminal-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const surfaceId = await app.eval({
    timeoutMs: 20_000,
    script: `
      const handle = await lx.shell.openDeclared('terminal', {
        key: ${JSON.stringify(token)},
        as: 'main',
      });
      globalThis.__terminalAutomationHandle = handle;
      return handle.id;
    `,
  }) as string;

  const terminal = lx.automation().terminal;
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
      expect(pane.frame.width > 20).toBeTruthy();
      expect(pane.frame.height > 20).toBeTruthy();
      expect(pane.grid.cols > 0).toBeTruthy();
      expect(pane.grid.rows > 0).toBeTruthy();
    }
  } finally {
    await app.eval({
      timeoutMs: 20_000,
      script: `
        const handle = globalThis.__terminalAutomationHandle;
        delete globalThis.__terminalAutomationHandle;
        if (handle?.alive) await handle.close();
      `,
    });
  }
});

desktopTerminalTest('keeps a maximized terminal maximized when a tab opens', async () => {
  const app = showcaseApp();
  const token = `automation-terminal-tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  // An aside is the shape that can be maximized: `main` already fills the
  // content area, so it could not show the state being clobbered.
  const surfaceId = await app.eval({
    timeoutMs: 20_000,
    script: `
      const handle = await lx.shell.openDeclared('terminal', {
        key: ${JSON.stringify(token)},
        as: 'aside',
        edge: 'bottom',
      });
      globalThis.__terminalTabAutomationHandle = handle;
      return handle.id;
    `,
  }) as string;

  const terminal = lx.automation().terminal;
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
    await app.eval({
      timeoutMs: 20_000,
      script: `
        const handle = globalThis.__terminalTabAutomationHandle;
        delete globalThis.__terminalTabAutomationHandle;
        if (handle?.alive) await handle.close();
      `,
    });
  }
});

desktopTerminalTest('applies terminal mode to native chrome before terminal input', async () => {
  const app = showcaseApp();
  const token = `automation-terminal-theme-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  // One surface per eval. Opening both in a single script reports only that
  // "eval timed out", which says nothing about which surface never settled —
  // and a native surface and a bundled lxapp settle along different paths.
  const terminalId = await app.eval({
    timeoutMs: 20_000,
    script: `
      const terminal = await lx.shell.openDeclared('terminal', {
        key: ${JSON.stringify(token)},
        as: 'main',
      });
      globalThis.__terminalThemeAutomationHandles = { terminal };
      return terminal.id;
    `,
  }) as string;
  // Opening a bundled lxapp aside has to reach the host and come back, so it
  // is slower than the native surface above and pays a cold start on CI.
  const settingsId = await app.eval({
    timeoutMs: 60_000,
    script: `
      const settings = await lx.shell.openApp('app.lingxia.terminal-settings', {
        as: 'aside',
        edge: 'right',
      });
      globalThis.__terminalThemeAutomationHandles.settings = settings;
      return settings.id;
    `,
  }) as string;
  const refs = { terminal: terminalId, settings: settingsId };
  const terminal = lx.automation().terminal;
  const settingsApp = lx.automation().lxapp('app.lingxia.terminal-settings');
  const page = settingsApp.page;
  let initial: TerminalWorkspaceSnapshot | undefined;

  try {
    // The action bar is intentionally hidden until the draft becomes dirty;
    // its button is still a reliable page-readiness marker once attached.
    await page.waitFor({ css: '#save', state: 'attached', timeoutMs: 10_000 });
    await page.waitFor({
      css: '[data-mode][aria-pressed="true"]',
      state: 'visible',
      timeoutMs: 10_000,
    });
    await waitForSave(page, false);
    const runtime = await settingsApp.eval({
      script: `
        const settings = await lx.terminal.settings.get();
        return {
          terminal: typeof lx.terminal?.settings?.get,
          fileSystem: typeof lx.fs,
          systemAppearance: settings.effective.systemAppearance,
        };
      `,
    }) as {
      terminal: string;
      fileSystem: string;
      systemAppearance: 'light' | 'dark';
    };
    expect(runtime.terminal).toBe('function');
    expect(runtime.fileSystem).toBe('undefined');
    initial = await terminal.snapshot({ surface: refs.terminal });
    const selectedMode = await page.eval({
      script: `document.querySelector('[data-mode][aria-pressed="true"]')?.dataset.mode`,
    }) as 'system' | 'light' | 'dark' | undefined;
    if (!selectedMode) throw new Error('selected terminal mode is unavailable');
    const effectiveMode = selectedMode === 'system'
      ? runtime.systemAppearance
      : selectedMode;
    const targetMode = effectiveMode === 'light' ? 'dark' : 'light';

    await page.click({ css: `[data-mode="${targetMode}"]` });
    await waitForSave(page, true);

    const previewed = await waitFor(async () => {
      const snapshot = await terminal.snapshot({ surface: refs.terminal });
      return snapshot.visualGeneration !== initial!.visualGeneration
        ? snapshot
        : undefined;
    }, 'native terminal preview chrome');
    expect(previewed.configGeneration).toBe(initial.configGeneration);

    await page.click({ css: '#save' });
    await waitForSave(page, false);
    const applied = await waitFor(async () => {
      const snapshot = await terminal.snapshot({ surface: refs.terminal });
      const config = snapshot.config as { theme?: { mode?: string } };
      return snapshot.configGeneration > initial!.configGeneration
        && config.theme?.mode === targetMode
        ? snapshot
        : undefined;
    }, 'persisted terminal mode');
    expect(applied.chrome.surface).toBe(previewed.chrome.surface);
    expect(applied.chrome.cursor).toBe(previewed.chrome.cursor);
  } finally {
    if (initial) {
      await settingsApp.eval({
        timeoutMs: 20_000,
        script: `
          const current = await lx.terminal.settings.get();
          await lx.terminal.settings.update(
            ${JSON.stringify(initial.config)},
            { ifRevision: current.revision },
          );
        `,
      });
    }
    await app.eval({
      timeoutMs: 20_000,
      script: `
        const handles = globalThis.__terminalThemeAutomationHandles;
        delete globalThis.__terminalThemeAutomationHandles;
        if (handles?.settings?.alive) await handles.settings.close();
        if (handles?.terminal?.alive) await handles.terminal.close();
      `,
    });
  }
});
