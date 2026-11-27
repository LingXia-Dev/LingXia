import assert from "node:assert/strict";
import { test } from "node:test";
import { deflateSync } from "node:zlib";
import { AssertionError, expect } from "../dist/index.js";
import { decodePng, dominantColour } from "../dist/png.js";

function crc32(bytes) {
  let crc = ~0;
  for (const byte of bytes) {
    crc ^= byte;
    for (let k = 0; k < 8; k++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return ~crc >>> 0;
}

function chunk(type, body) {
  const out = Buffer.alloc(12 + body.length);
  out.writeUInt32BE(body.length, 0);
  out.write(type, 4, "ascii");
  Buffer.from(body).copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + body.length)), 8 + body.length);
  return out;
}

/** A PNG of `pixels` (`channels` bytes each), row `y` filtered with `filters[y % filters.length]`. */
function encodePng({ width, height, channels, pixels }, { filters = [0], depth = 8, colour, interlace = 0 } = {}) {
  const stride = width * channels;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    const filter = filters[y % filters.length];
    raw[y * (stride + 1)] = filter;
    for (let i = 0; i < stride; i++) {
      const x = pixels[y * stride + i];
      const a = i >= channels ? pixels[y * stride + i - channels] : 0;
      const b = y > 0 ? pixels[(y - 1) * stride + i] : 0;
      const c = i >= channels && y > 0 ? pixels[(y - 1) * stride + i - channels] : 0;
      const p = a + b - c;
      const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
      const predictor = [0, a, b, (a + b) >> 1, pa <= pb && pa <= pc ? a : pb <= pc ? b : c][filter];
      raw[y * (stride + 1) + 1 + i] = (x - predictor) & 255;
    }
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = depth;
  header[9] = colour ?? (channels === 4 ? 6 : 2);
  header[12] = interlace;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function image(width, height, channels, paint) {
  const pixels = new Uint8Array(width * height * channels);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) pixels.set(paint(x, y).slice(0, channels), (y * width + x) * channels);
  }
  return { width, height, channels, pixels };
}

const shot = (png) => ({ format: "png", base64: Buffer.from(png).toString("base64"), width: 0, height: 0 });
const noisy = (x, y) => [(x * 37 + y * 11) & 255, (x * 5 + y * 101) & 255, (x * y * 3) & 255, (x + y * 7) & 255];

for (const channels of [3, 4]) {
  test(`decodes ${channels === 3 ? "RGB" : "RGBA"} rows under every filter type`, async () => {
    const source = image(23, 10, channels, noisy);
    const png = encodePng(source, { filters: [0, 1, 2, 3, 4] });
    const fromBytes = await decodePng(new Uint8Array(png));
    const fromBase64 = await decodePng(Buffer.from(png).toString("base64"));
    for (const decoded of [fromBytes, fromBase64]) {
      assert.equal(decoded.width, 23);
      assert.equal(decoded.height, 10);
      assert.equal(decoded.channels, channels);
      assert.deepEqual([...decoded.pixels], [...source.pixels]);
    }
  });
}

test("rejects a PNG it cannot decode, saying why", async () => {
  const source = image(4, 4, 3, noisy);
  await assert.rejects(decodePng(new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8])), /not a PNG/);
  await assert.rejects(decodePng(encodePng(source, { depth: 16 })), /bit depth 16, colour type 2/);
  await assert.rejects(decodePng(encodePng(source, { colour: 3 })), /colour type 3/);
  await assert.rejects(decodePng(encodePng(source, { colour: 0 })), /colour type 0/);
  await assert.rejects(decodePng(encodePng(source, { interlace: 1 })), /interlaced/);
  await assert.rejects(decodePng(encodePng(source, { filters: [5] })), /unknown filter type 5/);
});

test("dominantColour measures the share of one colour, within a tolerance", async () => {
  const decoded = await decodePng(encodePng(image(100, 100, 4, (x) => (x < 90 ? [250, 250, 250, 255] : [10, 20, 30, 255]))));
  const whole = dominantColour(decoded);
  assert.deepEqual(whole.colour, [250, 250, 250]);
  assert.equal(whole.samples, 10_000);
  assert.equal(whole.share, 0.9);
  assert.equal(dominantColour(decoded, { x: 0, y: 0, width: 90, height: 100 }).share, 1);
  assert.throws(() => dominantColour(decoded, { x: 50, y: 0, width: 60, height: 10 }), RangeError);
});

test("toShowContent fails a blank screenshot, naming its colour and its ink", async () => {
  const blank = shot(encodePng(image(200, 300, 4, () => [255, 255, 255, 255]), { filters: [2] }));
  const error = await expect(blank).toShowContent({ scale: 1 }).then(() => null, (error) => error);
  assert.ok(error instanceof AssertionError, String(error));
  assert.match(error.message, /blank: one colour, rgb\(255, 255, 255\) #ffffff/);
  assert.match(error.message, /Expected: at least 1500 pt² off one colour/);
  assert.match(error.message, /Received: 0 pt² off rgb\(255, 255, 255\) #ffffff, which is 100\.0% of 60000 sampled pixels/);
  await expect(blank).not.toShowContent({ scale: 1 });
});

test("toShowContent passes content, and reads only its region", async () => {
  const page = shot(encodePng(image(200, 300, 3, (x, y) => (y < 100 ? noisy(x, y) : [255, 255, 255]))));
  await expect(page).toShowContent({ scale: 1 });
  await expect(page).toShowContent({ scale: 1, region: { x: 0, y: 0, width: 200, height: 100 } });
  const error = await expect(page).toShowContent({ scale: 1, region: { x: 0, y: 100, width: 200, height: 200 } }).then(() => null, (error) => error);
  assert.match(error?.message ?? "", /region \{"x":0,"y":100,"width":200,"height":200\} is blank/);
});

test("toShowContent measures ink in points, so a sparse page passes in any window", async () => {
  // One 40x40 pt mark (1600 pt²) on a blank window, at 1x and 3x, in a small and a large window.
  const page = (side, scale) => shot(encodePng(image(side * scale, side * scale, 3,
    (x, y) => (x < 40 * scale && y < 40 * scale ? [30, 30, 30] : [246, 247, 251]))));
  await expect(page(400, 1)).toShowContent({ scale: 1 });
  await expect(page(400, 3)).toShowContent({ scale: 3 });
  await expect(page(800, 3)).toShowContent({ scale: 3 });
  // The same pixels read at 3 px/pt are a ninth of the area.
  await assert.rejects(expect(page(400, 1)).toShowContent({ scale: 3 }), /178 pt² off rgb\(246, 247, 251\)/);
  await assert.rejects(expect(page(400, 1)).toShowContent({ scale: 1, minInk: 2000 }), /at least 2000 pt²/);
});

test("toShowContent fails a blank dark page once the region leaves out the bars over it", async () => {
  // A 200x300 pt window at 2 px/pt: a 60 pt bar in a lighter dark tone over an empty dark canvas.
  const window = shot(encodePng(image(400, 600, 3, (x, y) => (y < 120 ? [27, 31, 42] : (y === 120 ? [58, 64, 80] : [17, 19, 26])))));
  await expect(window).toShowContent({ scale: 2 });
  const content = { x: 0, y: 122, width: 400, height: 478 };
  await assert.rejects(expect(window).toShowContent({ scale: 2, region: content }), /blank: one colour, rgb\(17, 19, 26\)[\s\S]*Received: 0 pt²/);
});

test("toShowContent still fails a blank page under a small badge or anti-aliased edges", async () => {
  // A 10x10 badge is 100 pt²: not content.
  const badge = shot(encodePng(image(200, 300, 3, (x, y) => (x < 10 && y < 10 ? [220, 40, 40] : [255, 255, 255]))));
  await assert.rejects(expect(badge).toShowContent({ scale: 1 }), AssertionError);
  // Near-white noise within the tolerance is still the one colour.
  const dither = shot(encodePng(image(200, 300, 3, (x, y) => [250 + ((x + y) % 5), 252, 255])));
  await assert.rejects(expect(dither).toShowContent({ scale: 1 }), /rgb\(25\d, 252, 255\)/);
});

test("toShowContent fails what is not a readable screenshot, even under .not", async () => {
  await assert.rejects(expect({ width: 1 }).toShowContent({ scale: 1 }), /must be a screenshot/);
  await assert.rejects(expect({ base64: "AAAA" }).not.toShowContent({ scale: 1 }), /not a PNG/);
  const page = shot(encodePng(image(10, 10, 3, noisy)));
  await assert.rejects(expect(page).toShowContent({ scale: 1, region: { x: 5, y: 5, width: 10, height: 10 } }), /not a non-empty rectangle/);
  await assert.rejects(expect(page).toShowContent(), /scale must be the screenshot's pixels per point/);
});
