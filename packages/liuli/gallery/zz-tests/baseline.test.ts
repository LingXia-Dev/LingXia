import { expect, spec } from '@lingxia/test';
import manifest from '../lxapp.json';

// The visual baseline's capture: every gallery page, at rest at its top, light and dark, as
// `<page>-<theme>.png`. Run by scripts/visual-baseline.mjs on the macOS Runner, once per size preset;
// not part of an ordinary test run.
spec.configure({ tags: ['baseline'] });

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
// Pages that move by themselves are left out: a video poster loads in its own time.
const moving = new Set(['video-player', 'video-controls', 'carousel', 'motion', 'skeleton']);
const pages = manifest.pages.map(page => page.name).filter(page => !moving.has(page));

// A picture is taken once the page has stopped changing: two shots in a row that are the same.
// A theme that has just been pinned, a picture still loading and a workspace settling all show as a
// frame that differs from the next.
async function settled(view: { settled(): Promise<void>; screenshot(): Promise<{ base64: string }> }) {
  await view.settled().catch(() => {});
  let shot = await view.screenshot();
  for (let tries = 0; tries < 8; tries += 1) {
    await pause(350);
    const next = await view.screenshot();
    if (next.base64 === shot.base64) return next;
    shot = next;
  }
  return shot;
}

for (const page of pages) {
  spec(`baseline: ${page}`, { start: { page }, timeout: 90_000 }, async (t) => {
    const view = t.app.view;
    // A page has a phone form and a desk form. A session that reports the wrong one for its window
    // would be recorded as a layout change, so the capture stops here instead.
    const form = (await view.viewport()).width >= 700 ? 'form-regular' : 'form-compact';
    await expect(view.testId(form), `the page has not taken its ${form === 'form-regular' ? 'desk' : 'phone'} form`).toBeAttached();
    for (const theme of ['light', 'dark'] as const) {
      // Held still, so a spinner or a sweep is caught at the same place every time.
      await view.environment({ theme, still: true });
      await pause(400);
      await t.attach(`${page}-${theme}.png`, (await settled(view)) as Awaited<ReturnType<typeof view.screenshot>>);
    }
  });
}
