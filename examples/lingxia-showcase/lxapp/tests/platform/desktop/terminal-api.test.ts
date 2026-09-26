import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, eventually } from '../../helpers/poll.js';
import type { AppSurface, TerminalSettingsApi } from '@lingxia/types';

/** What the settings probe keeps on the terminal app's Logic `globalThis`. */
interface TerminalProbeState {
  overrides: Parameters<TerminalSettingsApi['update']>[0] | null;
  off: (() => void) | null;
  events: number[];
}
type TerminalProbeStates = Record<string, TerminalProbeState | undefined>;

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
const terminalSpec = selectedGate ? spec.skip : spec;

/**
 * `lx.terminal` exists only inside the host-bundled Terminal Settings lxapp.
 * The Showcase host bundles it (lingxia.yaml), so the suite opens it and
 * evaluates there — the automation channel is not limited to the Showcase.
 */
const TERMINAL_APP_ID = 'app.lingxia.terminal-settings';
const PROBE_SCHEME = 'showcase-automation-probe';

interface SettingsRoundTrip {
  revision: number;
  size: number;
  defaultSize: number;
  hasOverrides: boolean;
  updated: { revision: number; size: number };
  stale: { code?: string } | null;
  reset: { revision: number; size: number; fontOverride: unknown };
  fonts: { count: number; monospace: boolean; family: string };
  schemes: { count: number; first: string; background: string };
  imported: { name: string; source: string; listed: boolean };
  preview: string;
  windows: string;
}

terminalSpec('read, revise, reset, and preview terminal settings inside the bundled settings lxapp', {
  id: 'TERMINAL-API-001',
  covers: [
    'lx.terminal',
    'lx.terminal.settings',
    'lx.terminal.settings.get',
    'lx.terminal.settings.update',
    'lx.terminal.settings.reset',
    'lx.terminal.settings.onChange',
    'lx.terminal.fonts',
    'lx.terminal.fonts.list',
    'lx.terminal.colorSchemes',
    'lx.terminal.colorSchemes.list',
    'lx.terminal.colorSchemes.import',
    'lx.terminal.colorSchemes.createPreview',
    'lx.terminal.windows',
    'lx.shell.openApp',
    'LxAppManager.close',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'TERMINAL-API-001');
  const platform = await runtimePlatform(app);
  if (!['macos', 'windows'].includes(platform)) {
    throw new Error(`terminal settings require macOS or Windows; got ${platform || 'unknown'}`);
  }
  const manager = t.automation.lxapps;
  const currentApp = async (): Promise<string> => (await manager.current()).appid;
  const stateKey = `__lingxiaTerminal_${namespace.replace(/-/g, '_')}`;

  // The automation manager resolves an lxapp from the dev directory, which a
  // host-bundled one is not in, so the shell is the way to open this.
  // Keep the handle: closing the lxapp leaves its entry in the main switcher,
  // and the surface handle is what closes the workspace with it.
  const surfaceKey = `${stateKey}_surface`;
  await app.logic.eval({ timeout: 20_000 }, async ({ lx }, key, appId) => {
    (globalThis as unknown as Record<string, AppSurface | undefined>)[key] =
      await lx.shell.openApp(appId, { as: 'main' });
  }, surfaceKey, TERMINAL_APP_ID);
  await eventually(currentApp, (appid) => appid === TERMINAL_APP_ID, {
    describe: 'terminal settings to become the current lxapp',
    timeoutMs: 20_000,
  });
  const terminal = t.apps.lxapp(TERMINAL_APP_ID);
  await eventually(() => terminal.logic.eval({ timeout: 5_000 }, () => true), (ready) => ready === true, {
    describe: 'terminal settings Logic runtime to answer',
    timeoutMs: 20_000,
    retryIf: () => true,
  });
  // Handing the workspace back is asserted at the end of the body, where it has
  // the case's own budget; the cleanup hook is only the safety net for a body
  // that failed earlier, and it must stay inside the short defer budget.
  defer(async () => {
    await manager.close({ app: TERMINAL_APP_ID }).catch(() => undefined);
  });

  await t.step('the ControlSurface follows host language updates', async () => {
    // The heading exists only in the settings document itself, not in the
    // blank document a WebView shows before its first navigation commits.
    await terminal.view.css('#type-heading', { page: 'settings' }).first().waitFor({ state: 'visible', timeout: 30_000 });
    const preference = await app.logic.eval(({ lx }) => lx.host.control!.displayLanguage.getPreference());
    try {
      for (const language of ['zh-CN', 'en-US']) {
        await app.logic.eval(async ({ lx }, language) => {
          await lx.host.control!.displayLanguage.setPreference(language);
        }, language);
        await eventually(
          // The bridge also writes the HTML language tag. Check translated
          // content to prove the screen re-rendered, not just that the
          // document was stamped.
          () => terminal.view.eval({ page: 'settings' }, ({ document }) => (
            document.querySelector('#type-heading')?.textContent
          )),
          (heading) => heading === (language === 'zh-CN' ? '字体' : 'Type'),
          { describe: `terminal settings to render ${language}`, timeoutMs: 10_000 },
        );
      }
    } finally {
      await app.logic.eval(async ({ lx }, preference) => {
        await lx.host.control!.displayLanguage.setPreference(preference);
      }, preference);
    }
  });

  const result: SettingsRoundTrip = await terminal.logic.eval({ timeout: 30_000 }, async ({ lx }, key, probeScheme) => {
    const t = lx.terminal!;
    const state: TerminalProbeState = { overrides: null, off: null, events: [] };
    (globalThis as unknown as TerminalProbeStates)[key] = state;

    const first = await t.settings.get();
    state.overrides = JSON.parse(JSON.stringify(first.overrides));
    state.off = t.settings.onChange((snapshot) => state.events.push(snapshot.revision));

    const nextSize = first.value.font.size + 1;
    const updated = await t.settings.update({ font: { size: nextSize } }, { ifRevision: first.revision });

    let stale = null;
    try {
      await t.settings.update({ font: { size: nextSize } }, { ifRevision: first.revision });
    } catch (error) {
      stale = { code: (error as { code?: string } | null)?.code };
    }

    const reset = await t.settings.reset({ ifRevision: updated.revision, scope: 'font' });

    const fonts = await t.fonts.list();
    const schemes = await t.colorSchemes.list();
    const source = schemes[0];
    const imported = await t.colorSchemes.import({
      text: JSON.stringify(source.scheme),
      name: probeScheme,
      overwrite: true,
    });
    const listed = (await t.colorSchemes.list()).some((entry) => entry.name === probeScheme);

    const preview = t.colorSchemes.createPreview();
    await preview.show(source.name);
    await preview.clear();
    await preview.close();

    return {
      revision: first.revision,
      size: first.value.font.size,
      defaultSize: first.defaults.font.size,
      hasOverrides: Object.keys(first.overrides).length > 0,
      updated: { revision: updated.revision, size: updated.value.font.size },
      stale,
      reset: { revision: reset.revision, size: reset.value.font.size, fontOverride: reset.overrides.font ?? null },
      fonts: { count: fonts.length, monospace: fonts.every((font) => typeof font.monospace === 'boolean'), family: fonts[0]?.family ?? '' },
      schemes: { count: schemes.length, first: source.name, background: source.scheme.background },
      imported: { name: imported.name, source: imported.source, listed },
      preview: 'shown, cleared, closed',
      windows: typeof t.windows,
    };
  }, stateKey, PROBE_SCHEME);

  await t.step('settings: revision-checked update, change event, stale rejection, scoped reset', async () => {
    expect(typeof result.revision).toBe('number');
    expect(result.updated.revision).toBe(result.revision + 1);
    expect(result.updated.size).toBe(result.size + 1);
    // onChange is delivered after the writes return and coalesces to the latest
    // snapshot, so the reset's revision is what a listener must eventually see.
    const events = await eventually(
      () => terminal.logic.eval((_, key) => (globalThis as unknown as TerminalProbeStates)[key]?.events ?? [], stateKey),
      (revisions) => revisions.includes(result.reset.revision),
      { describe: 'onChange to report the latest revision', timeoutMs: 10_000 },
    );
    expect(events.length).toBeGreaterThanOrEqual(1);
    expect(result.stale?.code).toBe('E_TERMINAL_REVISION_CONFLICT');
    expect(result.reset.revision).toBeGreaterThan(result.updated.revision);
    expect(result.reset.size).toBe(result.defaultSize);
    expect(result.reset.fontOverride).toBe(null);
  });

  await t.step('fonts and colour schemes are real inventories', async () => {
    expect(result.fonts.count).toBeGreaterThan(0);
    expect(result.fonts.monospace).toBe(true);
    expect(result.fonts.family.length).toBeGreaterThan(0);
    expect(result.schemes.count).toBeGreaterThan(0);
    expect(result.schemes.background.startsWith('#')).toBe(true);
    expect(result.imported.name).toBe(PROBE_SCHEME);
    expect(result.imported.source).toBe('imported');
    expect(result.imported.listed).toBe(true);
    expect(result.preview).toBe('shown, cleared, closed');
  });

  await t.step('the Windows-only integration namespace follows the platform', async () => {
    expect(result.windows).toBe(platform === 'windows' ? 'object' : 'undefined');
  });

  await t.step('hand the host back its own settings and its root main', async () => {
    await terminal.logic.eval({ timeout: 15_000 }, async ({ lx }, key) => {
      const states = globalThis as unknown as TerminalProbeStates;
      const state = states[key];
      state?.off?.();
      if (state?.overrides) {
        const latest = await lx.terminal!.settings.get();
        await lx.terminal!.settings.update(state.overrides, { ifRevision: latest.revision });
      }
      delete states[key];
    }, stateKey);
    await app.logic.eval({ timeout: 20_000 }, async (_, key) => {
      const surfaces = globalThis as unknown as Record<string, AppSurface | undefined>;
      const surface = surfaces[key];
      if (surface) await surface.close();
      delete surfaces[key];
    }, surfaceKey);
    await eventually(currentApp, (appid) => appid === SHOWCASE_APP_ID, {
      describe: 'showcase to be current again after closing terminal settings',
      timeoutMs: 15_000,
    });
    // Opening an lxapp as a main puts it in the host's main switcher, and the
    // current-lxapp answer returns to the Showcase before that switcher does.
    // Every workspace case after this one starts from the root main, so this
    // case has to hand it back — or say plainly that it could not.
    const layout = await eventually(
      () => app.surfaceLayout(),
      (snapshot) => snapshot.activeMainId === snapshot.mainSwitcher.rootSurfaceId,
      { describe: 'the root main to be active again after closing terminal settings', timeoutMs: 20_000 },
    );
    expect(layout.mains.includes(TERMINAL_APP_ID)).toBe(false);
  });
});
