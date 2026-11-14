import { execFile } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { pathToFileURL } from 'node:url';

const execute = promisify(execFile);
const coordinate = (value) => Number.isInteger(value) && value >= 0 && value <= 10000;

// An uncompressed 24- or 32-bit BMP, rows bottom-up unless the height is negative; as `[r, g, b]`.
// The Android fixture's `key` names, plus the keys a menu walk needs; values are ArkUI key codes.
const keys = {
  back: 'Back', enter: '2054', tab: '2049', escape: '2070', up: '2012', down: '2013', home: '2081', end: '2082',
  ...Object.fromEntries([...'abcdefghijklmnopqrstuvwxyz'].map((letter, index) => [letter, String(2017 + index)])),
};

export function bmpPixels(raw, points) {
  if (raw.toString('latin1', 0, 2) !== 'BM') throw new Error('not a BMP');
  const offset = raw.readUInt32LE(10), width = raw.readInt32LE(18), signed = raw.readInt32LE(22);
  const depth = raw.readUInt16LE(28), height = Math.abs(signed);
  if (depth !== 24 && depth !== 32) throw new Error(`unsupported BMP depth ${depth}`);
  const stride = Math.ceil((depth * width) / 32) * 4;
  return points.map(([x, y]) => {
    if (x >= width || y >= height) throw new Error('point outside the screen');
    const at = offset + (signed > 0 ? height - 1 - y : y) * stride + x * (depth / 8);
    return [raw[at + 2], raw[at + 1], raw[at]];
  });
}

// `uitest dumpLayout`: a tree of `{ attributes: { text, id, type, bounds: "[x1,y1][x2,y2]" }, children }`.
export function layoutNodes(json) {
  const nodes = [];
  const walk = (node) => {
    const attributes = node?.attributes ?? {};
    const bounds = String(attributes.bounds ?? '').match(/-?\d+/g)?.map(Number);
    if (bounds?.length === 4) {
      nodes.push({ text: String(attributes.text ?? ''), id: String(attributes.id ?? ''), type: String(attributes.type ?? ''), description: String(attributes.description ?? ''), bounds });
    }
    for (const child of node?.children ?? []) walk(child);
  };
  walk(JSON.parse(json));
  return nodes;
}

// Test-only OS input and screen reads, bound to one explicitly selected device. The random URL
// is a capability: pass it with --secret-arg harmonyDevice, never into an app. `screen()` returns
// the composited screen as a BMP; `hdc(args)` runs one hdc command against the device.
export function createHarmonyDeviceFixture({ hdc, screen }, token = randomBytes(24).toString('hex')) {
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
    // Free the device before answering: a caller's next request may arrive before `finally` runs.
    const done = (status, body) => {
      busy = false;
      reply(status, body);
    };
    try {
      const operation = path.slice(token.length + 2);
      if (request.method === 'GET' && operation === 'health') return done(200, { ok: true });
      if (request.method === 'GET' && operation === 'layout') {
        const remote = `/data/local/tmp/lingxia-layout-${token}.json`;
        try {
          await hdc(['shell', 'uitest', 'dumpLayout', '-p', remote]);
          return done(200, { nodes: layoutNodes(await hdc(['shell', 'cat', remote])) });
        } finally {
          await hdc(['shell', 'rm', '-f', remote]);
        }
      }
      if (request.method !== 'POST') return done(404, { error: 'unknown operation' });
      let body = '';
      for await (const chunk of request) {
        body += chunk;
        if (body.length > 2048) return done(413, { error: 'request too large' });
      }
      let input;
      try { input = JSON.parse(body); } catch { return done(400, { error: 'invalid JSON' }); }
      const points = Array.isArray(input?.points) && input.points.length >= 1 && input.points.length <= 32
        && input.points.every((point) => Array.isArray(point) && point.length === 2 && point.every(coordinate));
      if (operation === 'pixels' && points) {
        return done(200, { pixels: bmpPixels(await screen(), input.points) });
      }
      // The screen read while a finger rests at x, y: one operation, so nothing else runs mid-press.
      if (operation === 'heldPixels' && points && [input?.x, input?.y].every(coordinate)) {
        const held = hdc(['shell', 'uitest', 'uiInput', 'longClick', String(input.x), String(input.y)]);
        await new Promise((resolve) => setTimeout(resolve, 600));
        const pixels = bmpPixels(await screen(), input.points);
        await held;
        return done(200, { pixels });
      }
      // The system takes this as a user's finger, so an edge swipe is the system back gesture.
      if (operation === 'swipe' && [input?.x1, input?.y1, input?.x2, input?.y2].every(coordinate)
        && Number.isInteger(input?.velocity) && input.velocity >= 200 && input.velocity <= 40000) {
        await hdc(['shell', 'uitest', 'uiInput', 'swipe', ...[input.x1, input.y1, input.x2, input.y2, input.velocity].map(String)]);
        return done(200, { ok: true });
      }
      if (operation === 'click' && [input?.x, input?.y].every(coordinate)) {
        await hdc(['shell', 'uitest', 'uiInput', 'click', String(input.x), String(input.y)]);
        return done(200, { ok: true });
      }
      // A finger held still past the long-press delay; answers once it lifts.
      if (operation === 'longClick' && [input?.x, input?.y].every(coordinate)) {
        await hdc(['shell', 'uitest', 'uiInput', 'longClick', String(input.x), String(input.y)]);
        return done(200, { ok: true });
      }
      // A hardware key press through the system's input path.
      if (operation === 'key' && Object.hasOwn(keys, input?.key)) {
        await hdc(['shell', 'uitest', 'uiInput', 'keyEvent', keys[input.key]]);
        return done(200, { ok: true });
      }
      return done(400, { error: 'invalid input operation' });
    } catch (error) {
      if (!response.headersSent) done(500, { error: String(error) });
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
  const binary = process.argv.includes('--hdc') ? argument('--hdc') : 'hdc';
  const serial = argument('--device');
  const hdc = async (args) => (await execute(binary, ['-t', serial, ...args], { timeout: 15000, maxBuffer: 8 * 1024 * 1024, windowsHide: true })).stdout;
  // snapshot_display writes a JPEG; macOS `sips` turns it into a BMP this file can read.
  const screen = async () => {
    const dir = await mkdtemp(join(tmpdir(), 'lingxia-harmony-'));
    const remote = `/data/local/tmp/lingxia-screen-${process.pid}.jpeg`;
    try {
      await hdc(['shell', 'snapshot_display', '-f', remote]);
      await hdc(['file', 'recv', remote, join(dir, 'screen.jpeg')]);
      await execute('sips', ['-s', 'format', 'bmp', join(dir, 'screen.jpeg'), '--out', join(dir, 'screen.bmp')], { timeout: 15000 });
      return await readFile(join(dir, 'screen.bmp'));
    } finally {
      await hdc(['shell', 'rm', '-f', remote]).catch(() => {});
      await rm(dir, { recursive: true, force: true });
    }
  };
  const { server, token } = createHarmonyDeviceFixture({ hdc, screen });
  server.listen(process.argv.includes('--port') ? Number(argument('--port')) : 0, '127.0.0.1', () => {
    process.stdout.write(`http://127.0.0.1:${server.address().port}/${token}\n`);
  });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => server.close(() => process.exit(0)));
}
