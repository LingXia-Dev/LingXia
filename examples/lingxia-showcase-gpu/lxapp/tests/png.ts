/** RGB samples of an 8-bit, non-interlaced RGB or RGBA PNG, as screenshots return. */
export async function decodePng(base64: string): Promise<{ width: number; height: number; at(x: number, y: number): number[] }> {
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
