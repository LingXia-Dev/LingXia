/** A decoded 8-bit RGB or RGBA PNG: `pixels` holds `channels` bytes per pixel, row by row. */
export interface PngImage {
  width: number;
  height: number;
  channels: 3 | 4;
  pixels: Uint8Array;
}

/** A rectangle of a screenshot, in its pixels. */
export interface PixelRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The colour most of a region shows, and how much of it shows that colour. */
export interface DominantColour {
  /** `[r, g, b]`. */
  colour: [number, number, number];
  /** Share of the sampled pixels within `tolerance` of `colour`, 0 to 1. */
  share: number;
  /** How many pixels were sampled. */
  samples: number;
}

const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function fromBase64(text: string): Uint8Array {
  const table = new Int16Array(128).fill(-1);
  for (let i = 0; i < BASE64.length; i++) table[BASE64.charCodeAt(i)] = i;
  const clean = text.replace(/[\s=]+/g, "");
  const bytes = new Uint8Array(Math.floor((clean.length * 3) / 4));
  let out = 0, acc = 0, bits = 0;
  for (let i = 0; i < clean.length; i++) {
    const code = clean.charCodeAt(i);
    const value = code < 128 ? table[code] : -1;
    if (value < 0) throw new Error(`not base64: unexpected '${clean[i]}' at ${i}`);
    acc = ((acc << 6) | value) & 0xffffff;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes[out++] = (acc >> bits) & 255;
    }
  }
  return bytes.subarray(0, out);
}

async function inflate(chunks: Uint8Array<ArrayBuffer>[]): Promise<Uint8Array> {
  // DecompressionStream, not node:zlib: specs run in the app's test runtime, which has no Node built-ins.
  const stream = new DecompressionStream("deflate");
  const writer = stream.writable.getWriter();
  const written = (async () => {
    for (const chunk of chunks) await writer.write(chunk);
    await writer.close();
  })();
  const [raw] = await Promise.all([new Response(stream.readable).arrayBuffer(), written]);
  return new Uint8Array(raw);
}

/**
 * Decode an 8-bit, non-interlaced RGB (colour type 2) or RGBA (colour type 6) PNG, given as bytes
 * or base64 (a `Screenshot`'s `base64`). Any other PNG is rejected with the reason.
 */
export async function decodePng(png: Uint8Array | string): Promise<PngImage> {
  const bytes = typeof png === "string" ? fromBase64(png) : png;
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < 8 || signature.some((value, i) => bytes[i] !== value)) {
    throw new Error("not a PNG: the signature is missing");
  }
  const data = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let width = 0, height = 0, channels: 3 | 4 = 4;
  let header = false;
  const idat: Uint8Array<ArrayBuffer>[] = [];
  for (let at = 8; at + 8 <= bytes.length;) {
    const length = data.getUint32(at);
    const type = String.fromCharCode(bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]);
    if (at + 12 + length > bytes.length) throw new Error(`PNG is truncated in its ${type} chunk`);
    const body = bytes.subarray(at + 8, at + 8 + length);
    if (type === "IHDR") {
      width = data.getUint32(at + 8);
      height = data.getUint32(at + 12);
      const [depth, colour, , , interlace] = body.subarray(8, 13);
      if (depth !== 8 || (colour !== 2 && colour !== 6)) {
        throw new Error(`unsupported PNG: bit depth ${depth}, colour type ${colour}; only 8-bit RGB (2) and RGBA (6) decode`);
      }
      if (interlace !== 0) throw new Error("unsupported PNG: interlaced");
      channels = colour === 6 ? 4 : 3;
      header = true;
    } else if (type === "IDAT") {
      idat.push(body.slice());
    } else if (type === "IEND") {
      break;
    }
    at += 12 + length;
  }
  if (!header) throw new Error("not a PNG: no IHDR chunk");
  if (idat.length === 0) throw new Error("PNG has no image data");
  const raw = await inflate(idat);
  const stride = width * channels;
  if (raw.length < (stride + 1) * height) {
    throw new Error(`PNG image data is short: ${raw.length} bytes for ${width}x${height}`);
  }
  const pixels = new Uint8Array(stride * height);
  // One loop per filter type: a per-byte dispatch makes a phone-sized screenshot slow.
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const src = y * (stride + 1) + 1, out = y * stride, up = out - stride;
    if (filter === 0) {
      pixels.set(raw.subarray(src, src + stride), out);
    } else if (filter === 1) {
      for (let i = 0; i < stride; i++) pixels[out + i] = (raw[src + i] + (i >= channels ? pixels[out + i - channels] : 0)) & 255;
    } else if (filter === 2) {
      for (let i = 0; i < stride; i++) pixels[out + i] = (raw[src + i] + (y > 0 ? pixels[up + i] : 0)) & 255;
    } else if (filter === 3) {
      for (let i = 0; i < stride; i++) {
        const left = i >= channels ? pixels[out + i - channels] : 0;
        const above = y > 0 ? pixels[up + i] : 0;
        pixels[out + i] = (raw[src + i] + ((left + above) >> 1)) & 255;
      }
    } else if (filter === 4) {
      for (let i = 0; i < stride; i++) {
        const a = i >= channels ? pixels[out + i - channels] : 0;
        const b = y > 0 ? pixels[up + i] : 0;
        const c = i >= channels && y > 0 ? pixels[up + i - channels] : 0;
        const p = a + b - c;
        const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        pixels[out + i] = (raw[src + i] + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c)) & 255;
      }
    } else {
      throw new Error(`PNG row ${y} has unknown filter type ${filter}`);
    }
  }
  return { width, height, channels, pixels };
}

/**
 * The colour most of `region` shows and the share of it within `tolerance` (per channel) of that
 * colour. A large region is sampled on an even grid of about 250 000 points.
 */
export function dominantColour(image: PngImage, region?: PixelRegion, tolerance = 10): DominantColour {
  const area = region ?? { x: 0, y: 0, width: image.width, height: image.height };
  const { x, y, width, height } = area;
  if (![x, y, width, height].every(Number.isInteger) || width <= 0 || height <= 0
    || x < 0 || y < 0 || x + width > image.width || y + height > image.height) {
    throw new RangeError(`region ${JSON.stringify(area)} is not a non-empty rectangle inside the ${image.width}x${image.height} image`);
  }
  const step = Math.max(1, Math.ceil(Math.sqrt((width * height) / 250_000)));
  const { pixels, channels } = image;
  const at = (px: number, py: number) => (py * image.width + px) * channels;
  // Bucket by the top 5 bits of each channel, then measure exactly against the bucket's mean.
  const counts = new Uint32Array(1 << 15);
  const sums = new Float64Array(3 << 15);
  let samples = 0;
  for (let py = y; py < y + height; py += step) {
    for (let px = x; px < x + width; px += step) {
      const i = at(px, py);
      const key = ((pixels[i] >> 3) << 10) | ((pixels[i + 1] >> 3) << 5) | (pixels[i + 2] >> 3);
      counts[key]++;
      sums[key * 3] += pixels[i];
      sums[key * 3 + 1] += pixels[i + 1];
      sums[key * 3 + 2] += pixels[i + 2];
      samples++;
    }
  }
  let best = 0;
  for (let key = 1; key < counts.length; key++) if (counts[key] > counts[best]) best = key;
  const colour: [number, number, number] = [0, 1, 2].map((c) => Math.round(sums[best * 3 + c] / counts[best])) as [number, number, number];
  let near = 0;
  for (let py = y; py < y + height; py += step) {
    for (let px = x; px < x + width; px += step) {
      const i = at(px, py);
      if (Math.abs(pixels[i] - colour[0]) <= tolerance && Math.abs(pixels[i + 1] - colour[1]) <= tolerance
        && Math.abs(pixels[i + 2] - colour[2]) <= tolerance) near++;
    }
  }
  return { colour, share: near / samples, samples };
}
