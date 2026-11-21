/** RGB samples of an 8-bit, non-interlaced RGB or RGBA PNG, as screenshots return. Rows below
 * `lastRow(width)` and columns past `lastColumn(width)` are not unfiltered: a filter reads only the
 * pixels left of and above its own, so a check that samples near the top left pays for that part only. */
export async function decodePng(base64: string, lastRow: (width: number) => number = () => Infinity,
  lastColumn: (width: number) => number = () => Infinity): Promise<{ width: number; height: number; at(x: number, y: number): number[] }> {
  const table = new Uint8Array(128);
  'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'.split('').forEach((c, i) => { table[c.charCodeAt(0)] = i; });
  const clean = base64.replace(/=+$/, '');
  const bytes = new Uint8Array(Math.floor(clean.length * 3 / 4));
  for (let i = 0, o = 0, bits = 0, acc = 0; i < clean.length; i++) {
    acc = ((acc << 6) | table[clean.charCodeAt(i)]) & 0xffffff;
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
  const rows = Math.min(height, lastRow(width) + 1);
  const columns = Math.min(width, lastColumn(width) + 1);
  const span = columns * channels;
  const pixels = new Uint8Array(stride * rows);
  // One loop per filter: a per-byte dispatch is what made a phone-sized screenshot slow.
  for (let y = 0; y < rows; y++) {
    const filter = raw[y * (stride + 1)];
    const src = y * (stride + 1) + 1, out = y * stride, up = out - stride;
    if (filter === 0) for (let i = 0; i < span; i++) pixels[out + i] = raw[src + i];
    else if (filter === 1) for (let i = 0; i < span; i++) pixels[out + i] = (raw[src + i] + (i >= channels ? pixels[out + i - channels] : 0)) & 255;
    else if (filter === 2) for (let i = 0; i < span; i++) pixels[out + i] = (raw[src + i] + (y > 0 ? pixels[up + i] : 0)) & 255;
    else if (filter === 3) for (let i = 0; i < span; i++) pixels[out + i] = (raw[src + i] + (((i >= channels ? pixels[out + i - channels] : 0) + (y > 0 ? pixels[up + i] : 0)) >> 1)) & 255;
    else for (let i = 0; i < span; i++) {
      const a = i >= channels ? pixels[out + i - channels] : 0;
      const b = y > 0 ? pixels[up + i] : 0;
      const c = i >= channels && y > 0 ? pixels[up + i - channels] : 0;
      const p = a + b - c;
      const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
      pixels[out + i] = (raw[src + i] + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c)) & 255;
    }
  }
  return {
    width, height, at: (x, y) => {
      if (y >= rows) throw new Error(`row ${y} is past the decoded ${rows}`);
      if (x >= columns) throw new Error(`column ${x} is past the decoded ${columns}`);
      return [...pixels.subarray((y * width + x) * channels, (y * width + x) * channels + 3)];
    },
  };
}
