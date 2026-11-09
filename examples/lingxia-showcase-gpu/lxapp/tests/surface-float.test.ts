import { expect, spec } from '@lingxia/test';
import { rawAutomation } from '@lingxia/test/runner';
import { androidDevice, type AndroidNode } from '@lingxia/test/showcase/helpers/android-device.js';

const centre = (node: AndroidNode): [number, number] =>
  [Math.round((node.bounds[0] + node.bounds[2]) / 2), Math.round((node.bounds[1] + node.bounds[3]) / 2)];
const same = (a: number[], b: number[]) => a.every((channel, i) => Math.abs(channel - b[i]) <= 8);

// What the docs promise about a float: it is not part of the page that opened it, so it stays open
// while that page navigates, and it sits above the page's own top layer. Read from the composited
// screen, since each sits on its own surface.
spec('a float outlives its opener navigating away and stays above a page dialog', {
  id: 'SURFACE-FLOAT-STACK-001', covers: ['lx.surface.openPage', 'view:dialog'],
  requires: { args: ['androidDevice'] }, reason: 'reads the composited screen through the Android device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const app = t.app;
  const device = androidDevice(t);
  const key = `surface-float-stack-${Date.now()}`;
  try {
    await app.logic.eval(async ({ lx }, key) => {
      await lx.surface.openPage('surface', { as: 'float', key });
    }, key);
    await expect((await app.page({ name: 'surface' })).view.testId('surface-counter')).toBeVisible();

    await app.nav.to({ page: 'motion-lab' });
    await app.nav.back();
    expect(await app.logic.eval(({ lx }, key) => lx.surface.getByKey(key)?.alive === true, key)).toBe(true);
    await expect((await app.page({ name: 'surface' })).view.testId('surface-counter')).toBeVisible();

    const float = (await device.nodes()).filter(node => /:id\/surface-(query|show-count|hide-count|lifecycle)$/.test(node.resource));
    expect(float.length).toBeGreaterThanOrEqual(3);
    const left = Math.min(...float.map(node => node.bounds[0]));
    const middle = Math.round(float.reduce((sum, node) => sum + centre(node)[1], 0) / float.length);
    // A page point beside the float, which the full-width sheet below covers.
    expect(left).toBeGreaterThan(20);
    const page: [number, number] = [Math.round(left / 4), middle];
    const points = [...float.map(centre), page];
    const before = await device.pixels(points);

    const lab = (await app.page({ name: 'view-lab' })).view;
    await lab.testId('lab-tab-gesture').click();
    await lab.testId('lab-detents-open').click();
    await expect(lab.getByRole('dialog', { name: 'Places' })).toBeVisible();
    const after = await device.pixels(points);

    expect(same(before[before.length - 1], after[after.length - 1]), 'the page dialog shows where the float is not').toBe(false);
    for (let i = 0; i < float.length; i++)
      expect(same(before[i], after[i]), `float pixel ${i} unchanged by the page dialog`).toBe(true);
  } finally {
    await app.logic.eval(async ({ lx }, key) => {
      const surface = lx.surface.getByKey(key);
      if (surface?.kind === 'page') await surface.close();
    }, key);
  }
});

/** RGBA rows of an 8-bit, non-interlaced RGB or RGBA PNG. */
async function decodePng(base64: string): Promise<{ width: number; height: number; at(x: number, y: number): number[] }> {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  const clean = base64.replace(/=+$/, '');
  const bytes = new Uint8Array(Math.floor(clean.length * 3 / 4));
  for (let i = 0, o = 0, bits = 0, acc = 0; i < clean.length; i++) {
    acc = (acc << 6) | alphabet.indexOf(clean[i]);
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes[o++] = (acc >> bits) & 255;
    }
  }
  const view = new DataView(bytes.buffer);
  let width = 0, height = 0, channels = 4;
  const idat: Uint8Array[] = [];
  for (let at = 8; at < bytes.length;) {
    const length = view.getUint32(at);
    const type = String.fromCharCode(...bytes.subarray(at + 4, at + 8));
    const data = bytes.subarray(at + 8, at + 8 + length);
    if (type === 'IHDR') {
      width = view.getUint32(at + 8);
      height = view.getUint32(at + 12);
      if (data[8] !== 8 || data[12] !== 0 || (data[9] !== 6 && data[9] !== 2)) throw new Error(`unsupported PNG: depth ${data[8]} colour ${data[9]}`);
      channels = data[9] === 6 ? 4 : 3;
    } else if (type === 'IDAT') idat.push(data);
    at += 12 + length;
  }
  const inflate = new DecompressionStream('deflate');
  const writer = inflate.writable.getWriter();
  for (const chunk of idat) void writer.write(chunk);
  void writer.close();
  const raw = new Uint8Array(await new Response(inflate.readable).arrayBuffer());
  const stride = width * channels;
  const pixels = new Uint8Array(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const row = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? pixels[y * stride + i - channels] : 0;
      const b = y > 0 ? pixels[(y - 1) * stride + i] : 0;
      const c = i >= channels && y > 0 ? pixels[(y - 1) * stride + i - channels] : 0;
      const p = a + b - c;
      const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
      const predictor = [0, a, b, (a + b) >> 1, pa <= pb && pa <= pc ? a : pb <= pc ? b : c][filter];
      pixels[y * stride + i] = (row[i] + predictor) & 255;
    }
  }
  return { width, height, at: (x, y) => [...pixels.subarray((y * width + x) * channels, (y * width + x) * channels + 3)] };
}

// The host screenshot composites every window: a modal float shows its card, and around it the
// page under a scrim rather than the float window's opaque snapshot.
spec('a host screenshot shows the page dimmed around a modal float card', {
  id: 'SURFACE-FLOAT-SHOT-001', covers: ['lx.surface.openPage'],
  start: { page: 'view-lab' },
}, async (t) => {
  const app = t.app;
  const os = await app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os !== 'ios') t.skip('the iOS capture composites windows itself');
  const shot = async () => decodePng((await rawAutomation().lxapps.screenshot()).base64);
  const key = `surface-float-shot-${Date.now()}`;
  // Outside a centred 60% x 40% card: near the left edge, at several heights.
  const outside = (s: { width: number; height: number }) =>
    [0.15, 0.25, 0.75, 0.85].flatMap(y => [0.05, 0.1].map(x => [Math.round(s.width * x), Math.round(s.height * y)]));
  const before = await shot();
  const lit = outside(before).filter(([x, y]) => before.at(x, y).some(v => v > 60));
  expect(lit.length, 'the GPU page is in the screenshot, not black').toBeGreaterThan(4);
  try {
    await app.logic.eval(async ({ lx }, key) => {
      await lx.surface.openPage('surface', { as: 'float', key, size: { width: '60%', height: '40%' }, interaction: { modal: true } });
    }, key);
    await expect((await app.page({ name: 'surface' })).view.testId('surface-counter')).toBeAttached();
    await new Promise<void>(resolve => setTimeout(() => resolve(), 600));
    const after = await shot();
    for (const [x, y] of lit) {
      const dimmed = before.at(x, y).map(v => v * 0.55);
      const got = after.at(x, y);
      expect(got.every((v, i) => Math.abs(v - dimmed[i]) <= 16), `(${x},${y}) ${before.at(x, y)} -> ${got}, a 45% scrim over the page`).toBe(true);
    }
    const card = after.at(Math.round(after.width / 2), Math.round(after.height / 2));
    expect(card.some(v => v > 60), `the card's GPU page is drawn: ${card}`).toBe(true);
  } finally {
    await app.logic.eval(async ({ lx }, key) => {
      const surface = lx.surface.getByKey(key);
      if (surface?.kind === 'page') await surface.close();
    }, key);
  }
});
