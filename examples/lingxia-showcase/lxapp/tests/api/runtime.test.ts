import { expect } from '@rongjs/test';
import { contract } from '../support/contract.js';

contract({
  id: 'LOGIC-001',
  title: 'read core app, device, screen, network, and system state',
  covers: [
    'lx.getLxAppInfo',
    'lx.getDeviceInfo',
    'lx.getScreenInfo',
    'lx.getNetworkInfo',
    'lx.getSystemSetting',
    'lx.app.getBaseInfo',
    'lx.app.envVersion',
  ],
  layer: 'logic',
  levels: ['semantic', 'boundary'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
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

contract({
  id: 'LOGIC-002',
  title: 'register and remove portable runtime listeners',
  covers: [
    'lx.onNetworkChange',
    'lx.onDeviceOrientationChange',
    'lx.onKeyDown',
    'lx.onKeyUp',
    'lx.onWifiConnected',
  ],
  layer: 'logic',
  levels: ['semantic', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
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

contract({
  id: 'LOGIC-006',
  title: 'answer capability questions consistently with the optional members',
  covers: ['lx.supports'],
  layer: 'logic',
  levels: ['semantic', 'boundary'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
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
          { capability: 'nativeFileReview' },
        ].every((query) => typeof lx.supports(query) === 'boolean'),
      };
    `,
  }) as {
    terminalAgrees: boolean;
    autostartAgrees: boolean;
    rejectedUnknown: boolean;
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
  expect(result.rejectedMissingSurfaceValue).toBeTruthy();
  expect(result.rejectedFlagValue).toBeTruthy();
  expect(result.rejectedExtraField).toBeTruthy();
  expect(result.main).toBeTruthy();
  expect(result.float).toBeTruthy();
  expect(result.allBooleans).toBeTruthy();
});

contract({
  id: 'LOGIC-003',
  title: 'round-trip isolated key-value storage',
  covers: ['lx.getStorage', 'Storage.info', 'Storage.set', 'Storage.get', 'Storage.list', 'Storage.delete'],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, namespace }) => {
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

contract({
  id: 'LOGIC-004',
  title: 'round-trip files under lx user cache',
  covers: [
    'lx.getFileManager',
    'lx.env.USER_CACHE_PATH',
    'lx.env.USER_DATA_PATH',
    'FileManager.mkdir',
    'FileManager.writeFile',
    'FileManager.readFile',
    'FileManager.stat',
    'FileManager.rename',
    'FileManager.copyFile',
    'FileManager.exists',
    'FileManager.remove',
  ],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, namespace }) => {
  const result = await app.eval({
    script: `
      const files = lx.getFileManager();
      const root = lx.env.USER_CACHE_PATH + '/' + ${JSON.stringify(namespace)};
      const dataPathAvailable = typeof lx.env.USER_DATA_PATH === 'string' && lx.env.USER_DATA_PATH.length > 0;
      const source = root + '/source.txt';
      const renamed = root + '/renamed.txt';
      const copied = root + '/copied.txt';
      await files.mkdir({ path: root, recursive: true });
      try {
        await files.writeFile({ filePath: source, data: 'hello automation' });
        const text = await files.readFile({ filePath: source, encoding: 'utf8' });
        const stat = await files.stat({ path: source });
        await files.rename({ oldPath: source, newPath: renamed });
        await files.copyFile({ srcPath: renamed, destPath: copied });
        return {
          text: text.data,
          isFile: stat.isFile,
          renamed: await files.exists({ path: renamed }),
          copied: await files.exists({ path: copied }),
          dataPathAvailable,
        };
      } finally {
        await files.remove({ path: root, recursive: true });
      }
    `,
    timeoutMs: 15_000,
  }) as {
    text: string;
    isFile: boolean;
    renamed: boolean;
    copied: boolean;
    dataPathAvailable: boolean;
  };

  expect(result.text).toBe('hello automation');
  expect(result.isFile).toBeTruthy();
  expect(result.renamed).toBeTruthy();
  expect(result.copied).toBeTruthy();
  expect(result.dataPathAvailable).toBeTruthy();
});
