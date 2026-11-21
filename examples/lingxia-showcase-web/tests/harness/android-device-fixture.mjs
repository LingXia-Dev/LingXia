import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { promisify } from 'node:util';
import { pathToFileURL } from 'node:url';

const execute = promisify(execFile);
const keys = { back: 'KEYCODE_BACK', enter: 'KEYCODE_ENTER', tab: 'KEYCODE_TAB', escape: 'KEYCODE_ESCAPE' };
const coordinate = (value) => Number.isInteger(value) && value >= 0 && value <= 10000;

// `screencap` without -p: a width, height and format header (plus a colour space word on newer
// Android), then RGBA rows. The composited screen, every window and surface included.
export function screenPixels(raw, points) {
  const width = raw.readUInt32LE(0), height = raw.readUInt32LE(4);
  const header = raw.length - width * height * 4;
  if (header !== 12 && header !== 16) throw new Error('unexpected screencap layout');
  return points.map(([x, y]) => {
    if (x >= width || y >= height) throw new Error('point outside the screen');
    const at = header + (y * width + x) * 4;
    return [raw[at], raw[at + 1], raw[at + 2]];
  });
}

// Test-only OS input, bound to one explicitly selected device. The random URL
// is a capability: pass it with --secret-arg androidDevice, never into an app.
export function createAndroidDeviceFixture(runAdb, token = randomBytes(24).toString('hex')) {
  let busy = false;
  let injects;
  // The rotation settings before the first turn; `restore` puts back exactly these.
  let found;
  const setting = async (name) => (await runAdb(['shell', 'settings', 'get', 'system', name])).trim();
  const put = (name, value) => runAdb(['shell', 'settings', 'put', 'system', name, value]);
  const restoreRotation = async () => {
    if (!found) return;
    await put('user_rotation', found.user);
    await put('accelerometer_rotation', found.auto);
    found = undefined;
  };
  const server = createServer(async (request, response) => {
    const reply = (status, body) => {
      response.writeHead(status, { 'content-type': 'application/json' });
      response.end(JSON.stringify(body));
    };
    const path = request.url ?? '';
    if (!path.startsWith(`/${token}/`)) return reply(404, { error: 'not found' });
    if (busy) return reply(409, { error: 'device operation already running' });
    busy = true;
    try {
      const operation = path.slice(token.length + 2);
      if (request.method === 'GET' && operation === 'health') return reply(200, { ok: true });
      if (request.method === 'GET' && operation === 'hierarchy') {
        const remote = `/sdcard/lingxia-view-${token}.xml`;
        let dumped = false;
        for (let attempt = 0; attempt < 3; attempt++) {
          const result = await runAdb(['shell', 'uiautomator', 'dump', '--compressed', remote]);
          if (result.includes('UI hierchary dumped to:')) { dumped = true; break; }
        }
        if (!dumped) throw new Error('UIAutomator did not produce a fresh hierarchy');
        let xml;
        try {
          xml = await runAdb(['exec-out', 'cat', remote]);
        } finally {
          await runAdb(['shell', 'rm', remote]);
        }
        return reply(200, { xml });
      }
      // MIUI rejects injected input until "USB debugging (Security settings)" is on; probe once.
      if (request.method === 'GET' && operation === 'inject') {
        if (injects === undefined) {
          try {
            await runAdb(['shell', 'input', 'keyevent', 'KEYCODE_UNKNOWN']);
            injects = true;
          } catch (error) {
            if (!String(error).includes('INJECT_EVENTS')) throw error;
            injects = false;
          }
        }
        return reply(200, { ok: injects });
      }
      if (request.method === 'GET' && operation === 'size') {
        const sizes = [...(await runAdb(['shell', 'wm', 'size'])).matchAll(/(?:Physical|Override) size: (\d+)x(\d+)/g)];
        const size = sizes.at(-1);
        if (!size) throw new Error('Android display size unavailable');
        return reply(200, { width: Number(size[1]), height: Number(size[2]) });
      }
      if (request.method !== 'POST') return reply(404, { error: 'unknown operation' });
      let body = '';
      for await (const chunk of request) {
        body += chunk;
        if (body.length > 2048) return reply(413, { error: 'request too large' });
      }
      let input;
      try { input = JSON.parse(body); } catch { return reply(400, { error: 'invalid JSON' }); }
      if (operation === 'rotate' && ['portrait', 'landscape', 'restore'].includes(input?.orientation)) {
        // As a user turns the phone with auto-rotate off. The phone is someone's: `restore` puts
        // back the settings found before the first turn, never a guess such as auto-rotate on.
        if (input.orientation === 'restore') {
          await restoreRotation();
        } else {
          found ??= { auto: await setting('accelerometer_rotation'), user: await setting('user_rotation') };
          await put('accelerometer_rotation', '0');
          await put('user_rotation', input.orientation === 'landscape' ? '1' : '0');
        }
        return reply(200, { ok: true });
      }
      if (operation === 'pixels' && Array.isArray(input?.points) && input.points.length >= 1 && input.points.length <= 32
        && input.points.every((point) => Array.isArray(point) && point.length === 2 && point.every(coordinate))) {
        return reply(200, { pixels: screenPixels(await runAdb(['exec-out', 'screencap'], { binary: true }), input.points) });
      }
      let args;
      if (operation === 'key' && Object.hasOwn(keys, input?.key)) {
        args = ['keyevent', keys[input.key]];
      } else if (operation === 'tap' && coordinate(input?.x) && coordinate(input?.y)) {
        args = ['tap', String(input.x), String(input.y)];
      } else if (operation === 'swipe' && [input?.x1, input?.y1, input?.x2, input?.y2].every(coordinate)
        && Number.isInteger(input?.duration) && input.duration >= 100 && input.duration <= 2000) {
        args = ['swipe', String(input.x1), String(input.y1), String(input.x2), String(input.y2), String(input.duration)];
      } else {
        return reply(400, { error: 'invalid input operation' });
      }
      await runAdb(['shell', 'input', ...args]);
      reply(200, { ok: true });
    } catch (error) {
      if (!response.headersSent) reply(500, { error: String(error) });
      else response.destroy();
    } finally {
      busy = false;
    }
  });
  return { server, token, restoreRotation };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const argument = (name) => process.argv[process.argv.indexOf(name) + 1];
  if (!process.argv.includes('--device') || !argument('--device')) throw new Error('--device is required');
  const adb = process.argv.includes('--adb') ? argument('--adb') : 'adb';
  const serial = argument('--device');
  const { server, token, restoreRotation } = createAndroidDeviceFixture(async (args, { binary = false } = {}) => {
    const result = await execute(adb, ['-s', serial, ...args], { timeout: 15000, maxBuffer: (binary ? 64 : 2) * 1024 * 1024,
      windowsHide: true, ...(binary ? { encoding: 'buffer' } : {}) });
    return result.stdout;
  });
  server.listen(process.argv.includes('--port') ? Number(argument('--port')) : 0, '127.0.0.1', () => {
    process.stdout.write(`http://127.0.0.1:${server.address().port}/${token}\n`);
  });
  // A run cut short must not leave the phone turned.
  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => restoreRotation().catch(() => {}).finally(() => server.close(() => process.exit(0))));
  }
}
