import { expect, test } from '@lingxia/test';
import manifest from '../lxapp.json';

// The visual baseline's capture: every gallery page, at rest at its top, light and dark, as
// `<page>-<theme>.png`. Run by scripts/visual-baseline.mjs on the macOS Runner, once per size preset;
// not part of an ordinary test run.
test.configure({ tags: ['baseline'] });

const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
// Pages that move by themselves are left out: a video's poster and first frame come from the network
// in their own time, on its page and on its tile in the Media category.
const moving = new Set(['video-player', 'video-controls', 'category-media', 'carousel', 'motion', 'skeleton']);
// Left out until it opens the same way twice: on every other open its list rests at its start, not its end.
const unsteady = new Set<string>([]);
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

// A dialog, viewer, menu, sheet, tooltip, popover or hover card (both panels are non-modal dialogs)
// open over a page at rest was opened by something the capture did not do; its picture would be
// recorded as the page's. Named, so the refusal says which.
const overlayRoles = ['dialog', 'alertdialog', 'menu', 'tooltip'] as const;
async function openOverlay(view: { getByRole(role: (typeof overlayRoles)[number]): { count(): Promise<number>; nth(index: number): { isVisible(): Promise<boolean>; getAttribute(name: string): Promise<string | null> } } }) {
  for (const role of overlayRoles) {
    const all = view.getByRole(role);
    for (let index = 0, count = await all.count(); index < count; index += 1) {
      const one = all.nth(index);
      if (await one.isVisible().catch(() => false)) return `${role} "${(await one.getAttribute('aria-label').catch(() => null)) ?? ''}"`;
    }
  }
  return null;
}

for (const page of pages) {
  // Home opens on the entry the workspace last showed, which a page left open by the case before
  // hands it when the window widens; the home picture is the Overview, so it asks for that.
  const start = page === 'home' ? { page, params: { page: 'home' } } : { page };
  test(`baseline: ${page}`, { start, timeout: 90_000 }, async (t) => {
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
      // A kept page returns where it was left, and a theme change can move it; bring it back to its top.
      const top = view.testId('page-start');
      if (!last && (await top.count()) > 0) {
        await top.scrollIntoView();
        const [box, viewport] = [await top.boundingBox(), await view.viewport()];
        if (!box || box.y < viewport.top) throw Error(`${page} (${theme}): the page is not at its top`);
      }
      await pause(400);
      const shot = await settled(view);
      const open = await openOverlay(view);
      if (open) throw Error(`${page} (${theme}): a ${open} is open over the page; the capture opens nothing, so this picture is refused`);
      await t.attach(`${page}-${theme}.png`, shot as Awaited<ReturnType<typeof view.screenshot>>);
    }
  });
}
