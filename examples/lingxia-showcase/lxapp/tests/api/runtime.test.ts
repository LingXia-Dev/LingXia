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
    'lx.fs.file',
    'lx.fs.mkdir',
    'lx.fs.write',
    'lx.env.USER_CACHE_PATH',
    'lx.env.USER_DATA_PATH',
    'LxFile.text',
    'LxFile.json',
    'LxFile.base64',
    'LxFile.bytes',
    'LxFile.arrayBuffer',
    'LxFile.stat',
  ],
  layer: 'logic',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app, namespace }) => {
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
