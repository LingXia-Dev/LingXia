import { expect, spec } from '@lingxia/test';
import { bindFixture, evalCaught } from '../helpers/poll.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

spec("read core app, device, screen, network, and system state", { id: "LOGIC-001", covers: [
    'lx.getLxAppInfo',
    'lx.getDeviceInfo',
    'lx.getScreenInfo',
    'lx.getNetworkInfo',
    'lx.getSystemSetting',
    'lx.app.getBaseInfo',
    'lx.app.envVersion',
  ], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "LOGIC-001");

  const result = await app.eval({
    script: `
      const app = lx.getLxAppInfo();
      const device = lx.getDeviceInfo();
      const screen = lx.getScreenInfo();
      const network = await lx.getNetworkInfo();
      const system = lx.getSystemSetting();
      const host = lx.app.getBaseInfo();
      return {
        appId: app.appId,
        device: !!device.osName,
        screen: screen.width > 0 && screen.height > 0 && screen.scale > 0,
        network: typeof network.isConnected === 'boolean' && !!network.networkType,
        system: typeof system.wifiEnabled === 'boolean',
        host: !!host.os && !!host.productName,
        envVersion: lx.app.envVersion,
      };
    `,
  }) as {
    appId: string;
    device: boolean;
    screen: boolean;
    network: boolean;
    system: boolean;
    host: boolean;
    envVersion: string;
  };

  expect(result.appId).toBe('lingxia-showcase');
  expect(result.device).toBeTruthy();
  expect(result.screen).toBeTruthy();
  expect(result.network).toBeTruthy();
  expect(result.system).toBeTruthy();
  expect(result.host).toBeTruthy();
  expect(['developer', 'preview', 'release']).toContain(result.envVersion);
});

spec("register and remove portable runtime listeners", { id: "LOGIC-002", covers: [
    'lx.onNetworkChange',
    'lx.onDeviceOrientationChange',
    'lx.onKeyDown',
    'lx.onKeyUp',
    'lx.onWifiConnected',
  ], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "LOGIC-002");

  const result = await app.eval({
    script: `
      const callback = () => {};
      const unsubscribes = [
        lx.onNetworkChange(callback),
        lx.onDeviceOrientationChange(callback),
        lx.onKeyDown(callback),
        lx.onKeyUp(callback),
        lx.onWifiConnected(callback),
      ];
      if (unsubscribes.some((off) => typeof off !== 'function')) return false;
      unsubscribes.forEach((off) => off());
      // A spent handle is inert: calling it again must not disturb a later
      // subscription on the same function.
      const offAgain = lx.onDeviceOrientationChange(callback);
      unsubscribes.forEach((off) => off());
      offAgain();
      return true;
    `,
  });

  expect(result).toBeTruthy();
});

spec("answer capability questions consistently with the optional members", { id: "LOGIC-006", covers: ['lx.supports'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app } = bindFixture(t, "LOGIC-006");

  const result = await app.eval({
    script: `
      const terminalAgrees = ('terminal' in lx) === lx.supports({ capability: 'terminal' });
      const autostartAgrees = !!lx.app.autostart === lx.supports({ capability: 'autostart' });
      let rejectedUnknown = false;
      try {
        lx.supports({});
      } catch {
        rejectedUnknown = true;
      }
      const rejects = (query) => {
        try {
          lx.supports(query);
          return false;
        } catch {
          return true;
        }
      };
      return {
        terminalAgrees,
        autostartAgrees,
        rejectedUnknown,
        rejectedMissingSurfaceValue: rejects({ capability: 'surface' }),
        rejectedFlagValue: rejects({ capability: 'terminal', value: 'window' }),
        rejectedExtraField: rejects({ capability: 'browser', extra: true }),
        // Untyped callers can pass anything; a non-object has to reject the
        // same way a malformed one does, not through argument conversion.
        rejectedNonObject: ['surface.window', null, 42, undefined].every(rejects),
        // Placements every host can realize.
        main: lx.supports({ capability: 'surface', value: 'main' }),
        float: lx.supports({ capability: 'surface', value: 'float' }),
        // Answers are booleans, never undefined, for every declared capability.
        allBooleans: [
          { capability: 'surface', value: 'window' },
          { capability: 'surface', value: 'aside' },
          { capability: 'surface', value: 'tab' },
          { capability: 'notifications' }, { capability: 'browser' },
          { capability: 'proxy' }, { capability: 'selfUpdate' },
          { capability: 'process' },
          { capability: 'appUse' }, { capability: 'computerUse' },
          { capability: 'browserUse' },
        ].every((query) => typeof lx.supports(query) === 'boolean'),
      };
    `,
  }) as {
    terminalAgrees: boolean;
    autostartAgrees: boolean;
    rejectedUnknown: boolean;
    rejectedNonObject: boolean;
    rejectedMissingSurfaceValue: boolean;
    rejectedFlagValue: boolean;
    rejectedExtraField: boolean;
    main: boolean;
    float: boolean;
    allBooleans: boolean;
  };

  expect(result.terminalAgrees).toBeTruthy();
  expect(result.autostartAgrees).toBeTruthy();
  expect(result.rejectedUnknown).toBeTruthy();
  expect(result.rejectedNonObject).toBeTruthy();
  expect(result.rejectedMissingSurfaceValue).toBeTruthy();
  expect(result.rejectedFlagValue).toBeTruthy();
  expect(result.rejectedExtraField).toBeTruthy();
  expect(result.main).toBeTruthy();
  expect(result.float).toBeTruthy();
  expect(result.allBooleans).toBeTruthy();
});

spec("round-trip isolated key-value storage", { id: "LOGIC-003", covers: ['lx.getStorage', 'Storage.info', 'Storage.set', 'Storage.get', 'Storage.list', 'Storage.delete'], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, namespace } = bindFixture(t, "LOGIC-003");

  const result = await app.eval({
    script: `
      const storage = lx.getStorage();
      const key = ${JSON.stringify(namespace)};
      const before = await storage.info();
      let value;
      let present = false;
      try {
        await storage.set(key, { ok: true, count: 2 });
        value = await storage.get(key);
        present = (await storage.list()).includes(key);
      } finally {
        await storage.delete(key);
      }
      const after = await storage.info();
      return {
        value,
        present,
        removed: !(await storage.list()).includes(key),
        sizeRestored: after.keyCount === before.keyCount,
      };
    `,
  }) as {
    value: unknown;
    present: boolean;
    removed: boolean;
    sizeRestored: boolean;
  };

  expect(result.value).toEqual({ ok: true, count: 2 });
  expect(result.present).toBeTruthy();
  expect(result.removed).toBeTruthy();
  expect(result.sizeRestored).toBeTruthy();
});

spec("round-trip files under lx user cache", { id: "LOGIC-004", covers: [
    'lx.fs',
    'lx.fs.copy',
    'lx.fs.exists',
    'lx.fs.file',
    'lx.fs.mkdir',
    'lx.fs.remove',
    'lx.fs.rename',
    'lx.fs.write',
    'lx.env.USER_CACHE_PATH',
    'lx.env.USER_DATA_PATH',
    'LxFile.text',
    'LxFile.json',
    'LxFile.base64',
    'LxFile.bytes',
    'LxFile.arrayBuffer',
    'LxFile.stat',
  ], app: SHOWCASE_APP_ID }, async (t) => {
  const { app, namespace } = bindFixture(t, "LOGIC-004");

  const result = await app.eval({
    script: `
      const files = lx.fs;
      const root = lx.env.USER_CACHE_PATH + '/' + ${JSON.stringify(namespace)};
      const dataPathAvailable = typeof lx.env.USER_DATA_PATH === 'string' && lx.env.USER_DATA_PATH.length > 0;
      const source = root + '/source.txt';
      const renamed = root + '/renamed.txt';
      const copied = root + '/copied.txt';
      await files.mkdir(root, { recursive: true });
      try {
        const payload = JSON.stringify({ hello: 'automation' });
        await files.write(source, payload);
        const file = files.file(source);
        const text = await file.text();
        const json = await file.json();
        const bytes = await file.bytes();
        const buffer = await file.arrayBuffer();
        const base64 = await file.base64();
        const stat = await file.stat();
        await files.rename(source, renamed);
        await files.copy(renamed, copied);
        return {
          text,
          json,
          byteLength: bytes.byteLength,
          arrayBufferLength: buffer.byteLength,
          isUint8Array: bytes instanceof Uint8Array,
          base64,
          isFile: stat.isFile,
          renamed: await files.exists(renamed),
          copied: await files.exists(copied),
          dataPathAvailable,
        };
      } finally {
        await files.remove(root, { recursive: true });
      }
    `,
    timeoutMs: 15_000,
  }) as {
    text: string;
    json: { hello: string };
    byteLength: number;
    arrayBufferLength: number;
    isUint8Array: boolean;
    base64: string;
    isFile: boolean;
    renamed: boolean;
    copied: boolean;
    dataPathAvailable: boolean;
  };

  expect(result.text).toBe('{"hello":"automation"}');
  expect(result.json).toEqual({ hello: 'automation' });
  expect(result.byteLength).toBe(result.text.length);
  expect(result.arrayBufferLength).toBe(result.text.length);
  expect(result.isUint8Array).toBeTruthy();
  expect(result.base64).toBe('eyJoZWxsbyI6ImF1dG9tYXRpb24ifQ==');
  expect(result.isFile).toBeTruthy();
  expect(result.renamed).toBeTruthy();
  expect(result.copied).toBeTruthy();
  expect(result.dataPathAvailable).toBeTruthy();
});

spec("clear, prefix-list, missing vs null, and persist storage across reLaunch", {
  id: "LOGIC-007",
  covers: ['Storage.clear', 'Storage.list', 'Storage.get', 'Storage.set', 'lx.reLaunch'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace, defer } = bindFixture(t, "LOGIC-007");
  const persistKey = `${namespace}-persist`;
  defer(async () => {
    await app.eval({
      script: `
        const storage = lx.getStorage();
        const keys = await storage.list(${JSON.stringify(namespace)});
        for (const key of keys) await storage.delete(key);
      `,
    }).catch(() => undefined);
  });

  const first = await app.eval({
    script: `
      const storage = lx.getStorage();
      const ns = ${JSON.stringify(namespace)};
      await storage.set(ns + '-a', 1);
      await storage.set(ns + '-b', 2);
      await storage.set(ns + '-null', null);
      const prefixed = (await storage.list(ns)).sort();
      const storedNull = await storage.get(ns + '-null');
      const missing = await storage.get(ns + '-missing');
      const listedNull = prefixed.includes(ns + '-null');
      const listedMissing = prefixed.includes(ns + '-missing');
      await storage.set(${JSON.stringify(persistKey)}, { kept: true });
      await storage.clear();
      const afterClear = await storage.list(ns);
      const persistGone = await storage.get(${JSON.stringify(persistKey)});
      await storage.set(${JSON.stringify(persistKey)}, { kept: true });
      return { prefixed, storedNull, missing, listedNull, listedMissing, afterClear, persistGone };
    `,
  }) as {
    prefixed: string[];
    storedNull: unknown;
    missing: unknown;
    listedNull: boolean;
    listedMissing: boolean;
    afterClear: string[];
    persistGone: unknown;
  };

  expect(first.prefixed).toEqual([`${namespace}-a`, `${namespace}-b`, `${namespace}-null`].sort());
  expect(first.storedNull).toBe(null);
  expect(first.listedNull).toBeTruthy();
  expect(first.listedMissing).toBeFalsy();
  expect(first.missing == null).toBeTruthy();
  expect(first.afterClear).toEqual([]);
  expect(first.persistGone == null).toBeTruthy();

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home', 30_000);
  const persisted = await app.eval({
    script: `return await lx.getStorage().get(${JSON.stringify(persistKey)});`,
  });
  expect(persisted).toEqual({ kept: true });

  const oversized = await evalCaught(
    app,
    `await lx.getStorage().set(${JSON.stringify(namespace)} + '-big', 'x'.repeat(5 * 1024 * 1024 + 1));`,
  );
  expect(oversized.ok).toBeFalsy();
  expect(oversized.code).toBe('E_OUT_OF_RANGE');
  const leftover = await app.eval({
    script: `return await lx.getStorage().get(${JSON.stringify(namespace)} + '-big');`,
  });
  expect(leftover == null).toBeTruthy();
});

spec("read directories, LxFile.exists/path, and deny escaped paths", {
  id: "LOGIC-008",
  covers: ['lx.fs.readDir', 'lx.fs.stat', 'LxFile.exists', 'LxFile.path'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app, namespace } = bindFixture(t, "LOGIC-008");

  const result = await app.eval({
    script: `
      const files = lx.fs;
      const root = lx.env.USER_CACHE_PATH + '/' + ${JSON.stringify(namespace)};
      const child = root + '/child.txt';
      await files.mkdir(root, { recursive: true });
      try {
        await files.write(child, 'hi');
        let overwriteRejected = false;
        try {
          await files.write(child, 'nope');
        } catch {
          overwriteRejected = true;
        }
        await files.write(child, 'overwritten', { overwrite: true });
        const entries = await files.readDir(root);
        const file = files.file(child);
        const exists = await file.exists();
        const text = await file.text();
        const stat = await files.stat(root);
        return {
          names: entries.map((entry) => entry.name).sort(),
          childIsFile: entries.some((entry) => entry.name === 'child.txt' && entry.isFile),
          exists,
          path: file.path,
          text,
          dirStat: stat.isDirectory,
          overwriteRejected,
        };
      } finally {
        await files.remove(root, { recursive: true });
      }
    `,
  }) as {
    names: string[];
    childIsFile: boolean;
    exists: boolean;
    path: string;
    text: string;
    dirStat: boolean;
    overwriteRejected: boolean;
  };

  expect(result.childIsFile).toBeTruthy();
  expect(result.exists).toBeTruthy();
  expect(result.path).toContain(namespace);
  expect(result.text).toBe('overwritten');
  expect(result.dirStat).toBeTruthy();
  expect(result.overwriteRejected).toBeTruthy();

  const escaped = await evalCaught(app, `await lx.fs.stat('../secret');`);
  expect(escaped.ok).toBeFalsy();
  expect(escaped.code).toBe('E_INVALID_ARG');
  const absolute = await evalCaught(app, `await lx.fs.stat('C:/Windows/System32');`);
  expect(absolute.ok).toBeFalsy();
  expect(absolute.code).toBe('E_INVALID_ARG');
});
