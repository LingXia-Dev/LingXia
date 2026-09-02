import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID, showcaseApp } from '../../helpers/app.js';
import { runtimePlatform } from '../../helpers/platform.js';
import { bindFixture, evalCaught, eventually } from '../../helpers/poll.js';

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
    'lx.surface.get',
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
  const platform = await runtimePlatform(showcaseApp());
  if (!['macos', 'windows'].includes(platform)) {
    throw new Error(`browser tab surfaces require macOS or Windows; got ${platform || 'unknown'}`);
  }
  const browserOffered = await app.eval({
    script: `return !!lx.supports({ capability: 'surface', value: 'tab' })`,
  }) as boolean;
  expect(browserOffered).toBeTruthy();

  const browser = lx.automation().browser;
  const key = `${namespace}-tab`;
  const stateKey = `__lingxiaTabSurface_${namespace.replace(/-/g, '_')}`;
  const title = `fixture ${key}`;
  // openUrl accepts https or a file URL inside this lxapp's own directories;
  // the page is written through lx.fs so the whole round trip stays in-process.
  const relative = `${namespace}/tab.html`;
  const url = fileUrl((await app.info()).data_dir, relative);
  const readState = () => app.eval({
    script: `
      const state = globalThis[${JSON.stringify(stateKey)}];
      return {
        alive: !!state?.tab.alive,
        visible: !!state?.tab.visible,
        closed: state?.closed ?? 0,
        registered: lx.surface.get(${JSON.stringify(key)}) != null,
      };
    `,
  }) as Promise<TabState>;

  const tabsBefore = new Set((await browser.tabs()).map((tab) => tab.tab_id));
  defer(async () => {
    await app.eval({
      script: `
        const state = globalThis[${JSON.stringify(stateKey)}];
        if (state?.off) state.off();
        delete globalThis[${JSON.stringify(stateKey)}];
        await lx.fs.remove('lx://userdata/' + ${JSON.stringify(namespace)}, { recursive: true }).catch(() => undefined);
      `,
    }).catch(() => undefined);
    for (const tab of (await browser.tabs().catch(() => [])).filter((tab) => !tabsBefore.has(tab.tab_id))) {
      await browser.close({ tab: tab.tab_id }).catch(() => undefined);
    }
  });

  await app.eval({
    script: `
      await lx.fs.mkdir('lx://userdata/' + ${JSON.stringify(namespace)}, { recursive: true });
      await lx.fs.write('lx://userdata/' + ${JSON.stringify(relative)},
        '<!doctype html><html><head><meta charset="utf-8"><title>' + ${JSON.stringify(title)} + '</title></head>'
        + '<body><h1 data-fixture-page="' + ${JSON.stringify(key)} + '">' + ${JSON.stringify(key)} + '</h1></body></html>',
        { overwrite: true });
    `,
  });

  const opened = await app.eval({
    timeoutMs: 20_000,
    script: `
      const tab = await lx.surface.openUrl(${JSON.stringify(url)}, { as: 'tab', key: ${JSON.stringify(key)} });
      const state = { tab, closed: 0, off: null };
      state.off = tab.onClose(() => { state.closed += 1; });
      globalThis[${JSON.stringify(stateKey)}] = state;
      const registered = lx.surface.get(${JSON.stringify(key)});
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
    `,
  }) as OpenedTab;
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
      await app.eval({ script: `await globalThis[${JSON.stringify(stateKey)}].tab.activate();` });
      await app.eval({ timeoutMs: 15_000, script: `await globalThis[${JSON.stringify(stateKey)}].tab.close();` });
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
      const again = await evalCaught(app, `await globalThis[${JSON.stringify(stateKey)}].tab.close();`);
      expect(again.ok).toBeTruthy();
    });
  } else {
    await t.step('group scope rejects activate() and close() with unsupported_placement', async () => {
      for (const method of ['activate', 'close']) {
        const rejected = await evalCaught(app, `await globalThis[${JSON.stringify(stateKey)}].tab.${method}();`);
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
