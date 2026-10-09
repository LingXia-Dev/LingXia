import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { promisify } from 'node:util';
import { pathToFileURL } from 'node:url';

const execute = promisify(execFile);
const keys = { back: 'KEYCODE_BACK', enter: 'KEYCODE_ENTER', tab: 'KEYCODE_TAB', escape: 'KEYCODE_ESCAPE' };
const coordinate = (value) => Number.isInteger(value) && value >= 0 && value <= 10000;

// Test-only OS input, bound to one explicitly selected device. The random URL
// is a capability: pass it with --secret-arg androidDevice, never into an app.
export function createAndroidDeviceFixture(runAdb, token = randomBytes(24).toString('hex')) {
  let busy = false;
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
        const remote = `/sdcard/lingxia-ui-${token}.xml`;
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
  return { server, token };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const argument = (name) => process.argv[process.argv.indexOf(name) + 1];
  if (!process.argv.includes('--device') || !argument('--device')) throw new Error('--device is required');
  const adb = process.argv.includes('--adb') ? argument('--adb') : 'adb';
  const serial = argument('--device');
  const { server, token } = createAndroidDeviceFixture(async (args) => {
    const result = await execute(adb, ['-s', serial, ...args], { timeout: 15000, maxBuffer: 2 * 1024 * 1024, windowsHide: true });
    return result.stdout;
  });
  server.listen(process.argv.includes('--port') ? Number(argument('--port')) : 0, '127.0.0.1', () => {
    process.stdout.write(`http://127.0.0.1:${server.address().port}/${token}\n`);
  });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => server.close(() => process.exit(0)));
}
