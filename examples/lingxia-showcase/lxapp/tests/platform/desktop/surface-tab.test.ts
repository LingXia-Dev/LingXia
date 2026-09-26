import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, eventually, type Caught } from '../../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const selectedGate = testArgs.gate?.toLocaleLowerCase();
const tabSpec = selectedGate ? spec.skip : spec;

interface OpenedTab {
  kind: string;
  realized: string;
  scope: string;
  id: string;
  key: string | undefined;
  alive: boolean;
  visible: boolean;
  registered: boolean;
}

/** What the spec keeps on Logic's `globalThis` between evals. */
interface HeldTab {
  tab: { alive: boolean; visible: boolean; activate?(): Promise<void>; close?(): Promise<void> };
  closed: number;
  off: (() => void) | null;
}

interface TabState {
  alive: boolean;
  visible: boolean;
  closed: number;
  registered: boolean;
}

/** `file://` form of a native directory path, on either desktop path syntax. */
function fileUrl(nativeDir: string, relative: string): string {
  const forward = nativeDir.replace(/\\/g, '/');
  const absolute = forward.startsWith('/') ? forward : `/${forward}`;
  return `file://${encodeURI(`${absolute}/${relative}`)}`;
}

tabSpec('open a browser tab from Logic and control it through TabSurface', {
  id: 'DESKTOP-SURFACE-TAB-001',
  covers: [
    'lx.surface.openUrl',
    'lx.surface.getByKey',
    'TabSurface.kind',
    'TabSurface.realized',
    'TabSurface.scope',
    'TabSurface.id',
    'TabSurface.key',
    'TabSurface.alive',
    'TabSurface.visible',
    'TabSurface.activate',
    'TabSurface.close',
    'TabSurface.onClose',
    'BrowserDriver.tabs',
    'LxAppDriver.info',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, 'DESKTOP-SURFACE-TAB-001');
  const platform = await runtimePlatform(app);
  if (!['macos', 'windows'].includes(platform)) {
    throw new Error(`browser tab surfaces require macOS or Windows; got ${platform || 'unknown'}`);
  }
  const browserOffered = await app.logic.eval(({ lx }) => !!lx.supports('surface.tab'));
  expect(browserOffered).toBeTruthy();

  const browser = t.automation.browser;
  const key = `${namespace}-tab`;
  const stateKey = `__lingxiaTabSurface_${namespace.replace(/-/g, '_')}`;
  const title = `fixture ${key}`;
  // openUrl accepts https or a file URL inside this lxapp's own directories;
  // the page is written through lx.fs so the whole round trip stays in-process.
  const relative = `${namespace}/tab.html`;
  const url = fileUrl((await app.info()).data_dir, relative);
  const readState = (): Promise<TabState> => app.logic.eval(({ lx }, stateKey, key) => {
    const state = (globalThis as unknown as Record<string, HeldTab | undefined>)[stateKey];
    return {
      alive: !!state?.tab.alive,
      visible: !!state?.tab.visible,
      closed: state?.closed ?? 0,
      registered: lx.surface.getByKey(key) != null,
    };
  }, stateKey, key);

  const tabsBefore = new Set((await browser.tabs()).map((tab) => tab.tab_id));
  defer(async () => {
    await app.logic.eval(async ({ lx }, stateKey, namespace) => {
      const held = globalThis as unknown as Record<string, HeldTab | undefined>;
      const state = held[stateKey];
      if (state?.off) state.off();
      delete held[stateKey];
      await lx.fs.remove('lx://userdata/' + namespace, { recursive: true }).catch(() => undefined);
    }, stateKey, namespace).catch(() => undefined);
    for (const tab of (await browser.tabs().catch(() => [])).filter((tab) => !tabsBefore.has(tab.tab_id))) {
      await browser.close({ tab: tab.tab_id }).catch(() => undefined);
    }
  });

  await app.logic.eval(async ({ lx }, namespace, relative, title, key) => {
    await lx.fs.mkdir('lx://userdata/' + namespace, { recursive: true });
    await lx.fs.write('lx://userdata/' + relative,
      '<!doctype html><html><head><meta charset="utf-8"><title>' + title + '</title></head>'
      + '<body><h1 data-fixture-page="' + key + '">' + key + '</h1></body></html>',
      { overwrite: true });
  }, namespace, relative, title, key);

  const opened: OpenedTab = await app.logic.eval({ timeout: 20_000 }, async ({ lx }, url, key, stateKey) => {
    const tab = await lx.surface.openUrl(url, { as: 'tab', key });
    const state: HeldTab = { tab, closed: 0, off: null };
    // A group-scoped tab's type omits `onClose`, but a close by the browser
    // chrome still reaches it; this spec checks that it does.
    state.off = (tab as unknown as { onClose(handler: () => void): () => void })
      .onClose(() => { state.closed += 1; });
    (globalThis as unknown as Record<string, HeldTab>)[stateKey] = state;
    const registered = lx.surface.getByKey(key);
    return {
      kind: tab.kind,
      realized: tab.realized,
      scope: tab.scope,
      id: tab.id,
      key: tab.key,
      alive: tab.alive,
      visible: tab.visible,
      registered: registered != null && registered.id === tab.id,
    };
  }, url, key, stateKey);
  expect(opened.kind).toBe('tab');
  expect(['tab', 'aside']).toContain(opened.realized);
  expect(['tab', 'group']).toContain(opened.scope);
  expect(typeof opened.id).toBe('string');
  expect(opened.id.length).toBeGreaterThan(0);
  expect(opened.key).toBe(key);
  expect(opened.alive).toBeTruthy();
  expect(opened.visible).toBeTruthy();
  expect(opened.registered).toBeTruthy();

  // The handle describes a real browser tab, not just a registry entry.
  const tab = await eventually(
    async () => (await browser.tabs()).find((candidate) => (
      !tabsBefore.has(candidate.tab_id)
      && (candidate.title === title || (candidate.current_url ?? '').endsWith(encodeURI(relative)))
    )),
    (candidate) => candidate !== undefined,
    { describe: `browser tab for ${url}`, timeoutMs: 15_000 },
  );
  if (!tab) throw new Error('browser tab was not found');

  if (opened.scope === 'tab') {
    await t.step('activate() and close() act on the owned tab', async () => {
      await app.logic.eval(async (_, stateKey) => {
        await (globalThis as unknown as Record<string, HeldTab>)[stateKey].tab.activate?.();
      }, stateKey);
      await app.logic.eval({ timeout: 15_000 }, async (_, stateKey) => {
        await (globalThis as unknown as Record<string, HeldTab>)[stateKey].tab.close?.();
      }, stateKey);
      const state = await eventually(readState, (value) => !value.alive && value.closed >= 1 && !value.registered, {
        describe: 'closed tab surface to report dead, fire onClose, and leave the registry',
        timeoutMs: 10_000,
      });
      expect(state.closed).toBe(1);
      expect(state.visible).toBe(false);
      await eventually(
        async () => (await browser.tabs()).some((candidate) => candidate.tab_id === tab.tab_id),
        (present) => present === false,
        { describe: 'browser tab to disappear after close()', timeoutMs: 10_000 },
      );
      // Idempotent: a second close after success is not an error.
      const again: Caught = await app.logic.eval(async (_, stateKey) => {
        try {
          await (globalThis as unknown as Record<string, HeldTab>)[stateKey].tab.close?.();
          return { ok: true };
        } catch (error) {
          const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
          return { ok: false, code, message: String(message ?? error), data };
        }
      }, stateKey);
      expect(again.ok).toBeTruthy();
    });
  } else {
    await t.step('group scope rejects activate() and close() with unsupported_placement', async () => {
      for (const method of ['activate', 'close']) {
        // A group-scoped tab has no `activate`/`close` in its type: calling
        // them anyway is the contract under test.
        const rejected: Caught = await app.logic.eval(async (_, stateKey, method) => {
          const tab = (globalThis as unknown as Record<string, HeldTab>)[stateKey].tab as unknown as
            Record<string, () => Promise<void>>;
          try {
            await tab[method]();
            return { ok: true };
          } catch (error) {
            const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
            return { ok: false, code, message: String(message ?? error), data };
          }
        }, stateKey, method);
        expect(rejected.ok).toBeFalsy();
        expect((rejected.data as { reason?: string } | undefined)?.reason).toBe('unsupported_placement');
      }
      expect((await readState()).alive).toBeTruthy();
      await browser.close({ tab: tab.tab_id });
      const state = await eventually(readState, (value) => !value.alive && value.closed >= 1, {
        describe: 'chrome-owned tab close to reach the surface handle',
        timeoutMs: 10_000,
      });
      expect(state.closed).toBe(1);
    });
  }
});
