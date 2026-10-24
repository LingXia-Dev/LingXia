import assert from 'node:assert/strict';

// Product pages own the home route; renderer workloads live on their own page.
export async function openPlayground(shell, tree) {
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const bounds = node => node.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/).slice(1).map(Number);
  for (let attempt = 0; attempt < 12; attempt++) {
    const xml = tree();
    if (xml.includes(':id/say-hello"')) return;
    const link = xml.match(/<node[^>]*resource-id="[^"]+:id\/open-lab"[^>]*>/)?.[0];
    if (link) {
      const [left, top, right, bottom] = bounds(link);
      if (bottom - top >= 80) {
        shell('input', 'tap', String(Math.round((left + right) / 2)), String(Math.round((top + bottom) / 2)));
        await delay(800);
        continue;
      }
    }
    const scroll = xml.match(/<node[^>]*resource-id="[^"]+:id\/home-scroll"[^>]*>/)?.[0];
    assert.ok(scroll, 'home scroll must exist before entering the playground');
    const [left, top, right, bottom] = bounds(scroll);
    const x = String(Math.round((left + right) / 2));
    shell('input', 'swipe', x, String(bottom - 60), x, String(top + 60), '250');
    await delay(300);
  }
  throw new Error('GPU playground navigation timed out');
}
