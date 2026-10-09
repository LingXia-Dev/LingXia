import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../../helpers/app.js';

const args = globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {} as Record<string, string>;
const androidSpec = args.platform === 'android' ? spec : spec.skip;

androidSpec('render emoji, text symbols, and CJK without missing glyphs', {
  id: 'ANDROID-FONTS-001',
  app: SHOWCASE_APP_ID,
  start: { page: 'ui', query: { type: 'toast' } },
}, async (t) => {
  // Inspect rasterized glyphs: DOM text and nonzero text width also pass for tofu.
  const samples = await t.app.view.eval(({ document }) => {
    const canvas = document.createElement('canvas');
    canvas.width = 320;
    canvas.height = 100;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('2D canvas unavailable');
    ctx.font = '64px sans-serif';
    ctx.fillStyle = '#111111';
    return [
      '😀', '👍', '👍🏽', '👩', '👩‍💻', '👨‍👩‍👧‍👦', '🇨🇳', '🇺🇸', '❤️',
      '❤︎', '☀︎', '简', '體', 'あ', '한', '국', '어', 'A1', '\u{10ffff}',
    ].map((text) => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.fillText(text, 4, 78);
      const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      let colored = 0;
      let ink = 0;
      let rasterHash = 2166136261;
      for (const value of pixels) rasterHash = Math.imul(rasterHash ^ value, 16777619) >>> 0;
      for (let i = 0; i < pixels.length; i += 4) {
        if (pixels[i + 3] < 128) continue;
        ink++;
        if (Math.max(pixels[i], pixels[i + 1], pixels[i + 2])
            - Math.min(pixels[i], pixels[i + 1], pixels[i + 2]) > 30) colored++;
      }
      const span = document.createElement('span');
      span.textContent = text;
      span.style.cssText = 'position:fixed;font:64px sans-serif;white-space:pre';
      document.body.append(span);
      const width = span.getBoundingClientRect().width;
      span.remove();
      return { text, colored, ink, width, rasterHash, canvasWidth: ctx.measureText(text).width };
    });
  });
  await t.attach('emoji-rasterization', samples);
  for (const sample of samples.slice(0, 9)) {
    await t.step(sample.text, async () => {
      expect(sample.colored).toBeGreaterThan(100);
      // Check page text shaping in the DOM; canvas uses a separate run builder.
      expect(sample.width).toBeLessThan(samples[0].width * 1.2);
    });
  }
  const missingGlyph = samples[samples.length - 1];
  for (const sample of samples.slice(9, -1)) {
    await t.step(sample.text, async () => {
      expect(sample.ink).toBeGreaterThan(100);
      expect(sample.colored).toBe(0);
      expect(sample.rasterHash).not.toBe(missingGlyph.rasterHash);
    });
  }
});
