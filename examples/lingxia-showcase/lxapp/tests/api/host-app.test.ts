import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, evalCaught } from '../helpers/poll.js';

const testArgs = globalThis.__LINGXIA_AUTOMATION_HOST__?.args
  ?? {} as Record<string, string>;
// The published API intentionally omits autostart on mobile hosts. Keep the
// behavioral case for desktop hosts, where the login-item contract exists.
const MOBILE_HOSTS = new Set(['android', 'ios']);
const autostartSpec = MOBILE_HOSTS.has(testArgs.platform?.toLocaleLowerCase() ?? '')
  ? spec.skip
  : spec;

spec('publish the lxapp sandbox roots through lx.env', {
  id: 'ENV-001',
  covers: ['lx.env', 'lx.env.USER_DATA_PATH', 'lx.env.USER_CACHE_PATH'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'ENV-001');

  const env = await app.eval({
    script: `return {
      keys: Object.keys(lx.env).sort(),
      data: lx.env.USER_DATA_PATH,
      cache: lx.env.USER_CACHE_PATH,
      // A second read must return the same roots — env is a snapshot, not a probe.
      stable: lx.env.USER_DATA_PATH === lx.env.USER_DATA_PATH,
    };`,
  }) as { keys: string[]; data: string; cache: string; stable: boolean };

  expect(env.keys).toEqual(['USER_CACHE_PATH', 'USER_DATA_PATH']);
  expect(env.data).toMatch(/^lx:\/\//);
  expect(env.cache).toMatch(/^lx:\/\//);
  expect(env.data).not.toBe(env.cache);
  expect(env.stable).toBeTruthy();
});

spec('capture a host app screenshot into the lxapp sandbox', {
  id: 'HOSTAPP-SHOT-001',
  covers: ['lx.app', 'lx.app.screenshot'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SHOT-001');

  const shot = await app.eval({
    script: `
      const result = await lx.app.screenshot();
      const stat = await lx.fs.stat(result.tempFilePath);
      return {
        path: result.tempFilePath,
        width: result.width,
        height: result.height,
        bytes: stat.size,
      };
    `,
  }) as { path: string; width: number; height: number; bytes: number };
  t.defer(async () => {
    await app.eval({
      script: `try { await lx.fs.remove(${JSON.stringify(shot.path)}); } catch {} return true;`,
    });
  });

  // A screenshot the app cannot read back is not a screenshot.
  expect(shot.path).toMatch(/^lx:\/\//);
  expect(shot.width).toBeGreaterThan(0);
  expect(shot.height).toBeGreaterThan(0);
  expect(shot.bytes).toBeGreaterThan(0);
});

spec('set and clear the host app badge without leaving one behind', {
  id: 'HOSTAPP-BADGE-001',
  covers: ['lx.app.setBadge'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-001');
  t.defer(async () => {
    await app.eval({ script: `try { lx.app.setBadge(null); } catch {} return true;` });
  });

  const result = await app.eval({
    script: `
      lx.app.setBadge('7');
      lx.app.setBadge(12);
      lx.app.setBadge(null);
      // Clearing an already-clear badge must stay a no-op, not an error.
      lx.app.setBadge(null);
      lx.app.setBadge('');
      return 'ok';
    `,
  });

  expect(result).toBe('ok');
});

spec('reject a badge value that is neither a string, a number, nor null', {
  id: 'HOSTAPP-BADGE-002',
  covers: ['lx.app.setBadge', 'lx.tray.setBadge'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-BADGE-002');
  t.defer(async () => {
    await app.eval({ script: `try { lx.app.setBadge(null); lx.tray.setBadge(null); } catch {} return true;` });
  });

  // Coercing these would paint "[object Object]" on the dock instead of failing.
  for (const literal of ['{ text: \'7\' }', '[1, 2]', 'true', '() => {}']) {
    await t.step(`lx.app.setBadge(${literal})`, async () => {
      const outcome = await evalCaught(app, `lx.app.setBadge(${literal}); return 'accepted';`);
      expect(outcome.ok).toBeFalsy();
      expect(outcome.code).toBe('E_INVALID_ARG');
    });
    await t.step(`lx.tray.setBadge(${literal})`, async () => {
      const outcome = await evalCaught(app, `lx.tray.setBadge(${literal}); return 'accepted';`);
      expect(outcome.ok).toBeFalsy();
      expect(outcome.code).toBe('E_INVALID_ARG');
    });
  }
});

spec('answer checkUpdate with a decision instead of throwing', {
  id: 'HOSTAPP-UPDATE-001',
  covers: ['lx.app.checkUpdate', 'lx.getUpdateManager', 'UpdateManager.onUpdateReady', 'UpdateManager.onUpdateFailed'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-UPDATE-001');

  const result = await app.eval({
    script: `
      const decision = await lx.app.checkUpdate();
      const manager = lx.getUpdateManager();
      const ready = manager.onUpdateReady(() => {});
      const failed = manager.onUpdateFailed(() => {});
      ready();
      failed();
      // Unsubscribing twice must stay inert.
      ready();
      return {
        hasUpdate: decision.hasUpdate,
        version: decision.version ?? null,
        subscriptions: [typeof ready, typeof failed],
      };
    `,
  }) as { hasUpdate: boolean; version: string | null; subscriptions: string[] };

  expect(typeof result.hasUpdate).toBe('boolean');
  expect(result.subscriptions).toEqual(['function', 'function']);
  if (result.hasUpdate) expect(typeof result.version).toBe('string');
  else expect(result.version).toBe(null);
});

spec('reject an invalid host display language', {
  id: 'HOSTAPP-LANG-002',
  covers: ['lx.app.setDisplayLanguage'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-LANG-002');

  const offered = await app.eval({
    script: `return typeof lx.app.setDisplayLanguage`,
  }) as string;
  expect(offered).toBe('function');

  for (const language of ['', 'ja-JP']) {
    const rejected = await evalCaught(
      app,
      `lx.app.setDisplayLanguage(${JSON.stringify(language)})`,
    );
    expect(rejected.ok).toBe(false);
    expect(String(rejected.code)).toBe('E_INVALID_ARG');
  }
});

spec('subscribe to and release the display language listener', {
  id: 'HOSTAPP-LANG-001',
  covers: ['lx.app.onDisplayLanguageChange'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-LANG-001');

  const result = await app.eval({
    script: `
      const first = lx.app.onDisplayLanguageChange(() => {});
      const second = lx.app.onDisplayLanguageChange(() => {});
      first();
      second();
      first();
      return { kinds: [typeof first, typeof second], distinct: first !== second };
    `,
  }) as { kinds: string[]; distinct: boolean };

  expect(result.kinds).toEqual(['function', 'function']);
  expect(result.distinct).toBeTruthy();
});

autostartSpec('report autostart state and accept an idempotent write', {
  id: 'HOSTAPP-AUTOSTART-001',
  covers: ['lx.app.autostart', 'lx.app.autostart.isEnabled', 'lx.app.autostart.setEnabled'],
  app: SHOWCASE_APP_ID,
  reason: 'Autostart is intentionally absent on mobile hosts.',
  // `SMAppService.mainApp.status` costs ~6s per call on macOS; the spec pays
  // that twice rather than pretending the API is cheap.
  timeout: 90_000,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-AUTOSTART-001');

  // Autostart is a login-item concept the inventory marks optional; a phone
  // host does not build it, and asking is how a caller finds out.
  const offered = await app.eval({
    script: `return !!(lx.app.autostart && typeof lx.app.autostart.isEnabled === 'function')`,
  }) as boolean;
  if (!offered) {
    const supported = await app.eval({ script: `return !!lx.supports({ capability: 'autostart' })` });
    expect(supported).toBe(false);
    return;
  }

  // Writing the value the host already holds proves the setter without
  // registering or removing a real login item on the developer's machine.
  const result = await app.eval({
    // The macOS login-item service answers well past the 5s eval default.
    timeoutMs: 45_000,
    script: `
      const before = await lx.app.autostart.isEnabled();
      await lx.app.autostart.setEnabled(before);
      const after = await lx.app.autostart.isEnabled();
      return { before, after };
    `,
  }) as { before: boolean; after: boolean };

  expect(typeof result.before).toBe('boolean');
  expect(result.after).toBe(result.before);
});

spec('show, label, and retract the host tray item', {
  id: 'HOSTAPP-TRAY-001',
  covers: [
    'lx.tray',
    'lx.tray.show',
    'lx.tray.hide',
    'lx.tray.setTitle',
    'lx.tray.setBadge',
    'lx.tray.setMenu',
    'lx.tray.setIcon',
    'lx.tray.onClick',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-TRAY-001');
  t.defer(async () => {
    await app.eval({ script: `try { lx.tray.hide(); } catch {} return true;` });
  });

  const result = await app.eval({
    script: `
      lx.tray.show();
      lx.tray.setTitle('LX');
      lx.tray.setIcon('public/showcase-icon.svg');
      lx.tray.setBadge('3');
      lx.tray.setMenu([
        { label: 'Open Showcase', onClick: () => {} },
        { separator: true },
        { label: 'Disabled', enabled: false },
      ]);
      const off = lx.tray.onClick(() => {});
      off();
      lx.tray.setBadge(null);
      lx.tray.setTitle(null);
      lx.tray.hide();
      // Hiding a hidden tray stays a no-op.
      lx.tray.hide();
      return { unsubscribe: typeof off };
    `,
  }) as { unsubscribe: string };

  expect(result.unsubscribe).toBe('function');
});

spec('declare, patch, and retract runtime sidebar actions atomically', {
  id: 'HOSTAPP-SIDEBAR-001',
  covers: [
    'lx.shell',
    'lx.shell.sidebarActions',
    'lx.shell.sidebarActions.replace',
    'lx.shell.sidebarActions.update',
    'lx.shell.sidebarActions.remove',
    'lx.shell.sidebarActions.clear',
  ],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SIDEBAR-001');
  // The declaration is process-local shell chrome; drop it however the spec ends.
  t.defer(async () => {
    await app.eval({ script: `try { lx.shell.sidebarActions.clear(); } catch {} return true;` });
  });

  await t.step('declare a header and a footer action', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.replace([
        { id: 'probe-header', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Probe header', onActivate() {} },
        { id: 'probe-footer', placement: 'footer', icon: 'public/showcase-icon.svg', label: 'Probe footer', onActivate() {} },
      ]);
      return 'declared';
    `);
    expect(result.ok).toBeTruthy();
  });

  await t.step('patch presentation of one live id', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.update('probe-footer', { label: 'Probe footer 2', disabled: true });
      return 'patched';
    `);
    expect(result.ok).toBeTruthy();
  });

  await t.step('reject a patch for an id outside the declaration', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.update('probe-missing', { label: 'nope' });
      return 'patched';
    `);
    expect(result.ok).toBeFalsy();
    expect(result.code).toBe('E_NOT_FOUND');
  });

  await t.step('reject a third header action without disturbing the live set', async () => {
    const result = await evalCaught(app, `
      lx.shell.sidebarActions.replace([
        { id: 'h1', placement: 'header', icon: 'public/showcase-icon.svg', label: 'One', onActivate() {} },
        { id: 'h2', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Two', onActivate() {} },
        { id: 'h3', placement: 'header', icon: 'public/showcase-icon.svg', label: 'Three', onActivate() {} },
      ]);
      return 'declared';
    `);
    expect(result.ok).toBeFalsy();
    // The rejected generation must not have replaced the live one.
    const survivor = await evalCaught(app, `
      lx.shell.sidebarActions.update('probe-header', { label: 'Probe header 2' });
      return 'patched';
    `);
    expect(survivor.ok).toBeTruthy();
  });

  await t.step('remove one id, then clear the rest idempotently', async () => {
    const removed = await evalCaught(app, `lx.shell.sidebarActions.remove('probe-footer'); return 'removed';`);
    expect(removed.ok).toBeTruthy();

    const removedTwice = await evalCaught(app, `lx.shell.sidebarActions.remove('probe-footer'); return 'removed';`);
    expect(removedTwice.ok).toBeFalsy();
    expect(removedTwice.code).toBe('E_NOT_FOUND');

    const cleared = await evalCaught(app, `lx.shell.sidebarActions.clear(); lx.shell.sidebarActions.clear(); return 'cleared';`);
    expect(cleared.ok).toBeTruthy();
  });
});

spec('reject shell surface reconfigure for an id the shell never realized', {
  id: 'HOSTAPP-SHELL-RECONFIG-001',
  covers: ['lx.shell.reconfigure'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SHELL-RECONFIG-001');

  const result = await evalCaught(app, `
    await lx.shell.reconfigure('surface-that-was-never-opened', { as: 'aside', edge: 'trailing' });
    return 'reconfigured';
  `);

  expect(result.ok).toBeFalsy();
  expect(typeof result.code).toBe('string');
  expect(String(result.code).length).toBeGreaterThan(0);
});

spec('subscribe to and release the surface context listener', {
  id: 'HOSTAPP-SURFACE-CTX-001',
  covers: ['lx.surface', 'lx.surface.onContext'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, 'HOSTAPP-SURFACE-CTX-001');

  const result = await app.eval({
    script: `
      const first = lx.surface.onContext(() => {});
      const second = lx.surface.onContext(() => {});
      first();
      second();
      // Releasing twice must stay inert rather than throwing.
      first();
      return { kinds: [typeof first, typeof second], distinct: first !== second };
    `,
  }) as { kinds: string[]; distinct: boolean };

  expect(result.kinds).toEqual(['function', 'function']);
  expect(result.distinct).toBeTruthy();
});
