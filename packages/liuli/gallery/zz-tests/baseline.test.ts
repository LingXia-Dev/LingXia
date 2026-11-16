import { expect, spec } from '@lingxia/test';
import manifest from '../lxapp.json';

// The visual baseline's capture: every gallery page, at rest at its top, light and dark, as
// `<page>-<theme>.png`. Run by scripts/visual-baseline.mjs on the macOS Runner, once per size preset;
// not part of an ordinary test run.
spec.configure({ tags: ['baseline'] });

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
// Pages that move by themselves are left out: a video poster loads in its own time.
const moving = new Set(['video-player', 'video-controls', 'carousel', 'motion', 'skeleton']);
// Left out until it opens the same way twice: on every other open its list rests at its start, not its end.
const unsteady = new Set(['conversation']);
const pages = manifest.pages.map(page => page.name).filter(page => !moving.has(page) && !unsteady.has(page));
// Pages whose list opens at its end, and a row that is drawn only once it has.
const atEnd: Record<string, string | undefined> = { conversation: 'retry-m8' };

// A picture is taken once the page has stopped changing: three shots in a row that are the same.
// A picture still being drawn at its final quality and a workspace settling show as a frame that
// differs from the next; two alike were not enough for a page of photographs.
async function settled(view: { settled(): Promise<void>; screenshot(): Promise<{ base64: string }> }) {
  await view.settled().catch(() => {});
  let shot = await view.screenshot();
  let alike = 0;
  for (let tries = 0; tries < 14; tries += 1) {
    await pause(350);
    const next = await view.screenshot();
    alike = next.base64 === shot.base64 ? alike + 1 : 0;
    shot = next;
    if (alike === 2) return shot;
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
      // The page says which appearance it is drawn in: a shot taken before the pinned one has taken is the other one's picture.
      await expect(view.testId(`page-appearance-${theme}`), `the page has not turned ${theme}`).toBeAttached();
      // A list that opens at its end is at rest once its last row is on screen; a row can exist before it is.
      const last = atEnd[page];
      if (last) {
        let shown = false;
        for (let tries = 0; tries < 20 && !shown; tries += 1) {
          const [box, viewport] = [await view.testId(last).boundingBox().catch(() => null), await view.viewport()];
          shown = box !== null && box.y >= viewport.top && box.y + box.height <= viewport.top + viewport.height;
          if (!shown) await pause(250);
        }
        if (!shown) throw Error(`${page} has not come to rest at its end: its last row is not on screen`);
      }
      await pause(400);
      await t.attach(`${page}-${theme}.png`, (await settled(view)) as Awaited<ReturnType<typeof view.screenshot>>);
    }
  });
}
