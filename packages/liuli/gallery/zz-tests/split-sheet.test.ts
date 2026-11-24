import { expect, test } from '@lingxia/test';

// On a phone, Split's aside opens as a sheet, and opens whole: at the half detent its lower part,
// where a reply field sits, is below the screen. Run at the phone size preset.

for (const theme of ['light', 'dark'] as const) {
  test(`split sheet: the sheet opens whole, its reply field in view (${theme})`, { start: { page: 'split' } }, async (tc) => {
    const view = tc.app.view;
    await view.environment({ theme, still: true });
    const open = view.testId('doc-open-comments');
    await open.scrollIntoView();
    await open.click();
    const reply = view.testId('doc-reply');
    await reply.waitFor({ state: 'visible' });
    const viewport = await view.viewport();
    const bottom = viewport.top + viewport.height;
    const inView = async (testId: string) => {
      const box = await view.testId(testId).boundingBox();
      return box.y >= viewport.top && box.y + box.height <= bottom + 1;
    };
    await expect.poll(() => inView('doc-split-sheet')).toBe(true);
    expect(await inView('doc-reply')).toBe(true);
    await tc.attach(`split-sheet-${theme}.png`, await view.screenshot());
    await view.testId('doc-split-sheet').performAction('dismiss');
  });
}
