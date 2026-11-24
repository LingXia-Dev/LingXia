import { expect, test } from '@lingxia/test';

// On a phone, Split's aside opens as a sheet, and opens whole: at the half detent its lower part,
// where a reply field sits, is below the screen. The field is pinned last, at the sheet's bottom
// above the home indicator, not after the comments. Run at the phone size preset.

for (const theme of ['light', 'dark'] as const) {
  test(`split sheet: the sheet opens whole, its reply field pinned at its bottom (${theme})`, { start: { page: 'split' } }, async (tc) => {
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
    await tc.attach(`split-sheet-${theme}.png`, await view.screenshot());
    expect(await inView('doc-reply')).toBe(true);
    const inset = (await view.testId('safe-area-bottom').boundingBox()).height - 1;
    const [sheet, field] = [await view.testId('doc-split-sheet').boundingBox(), await view.testId('doc-reply-row').boundingBox()];
    const gap = sheet.y + sheet.height - inset - (field.y + field.height);
    expect(gap).toBeGreaterThanOrEqual(0);
    expect(gap).toBeLessThanOrEqual(24);
    await view.testId('doc-split-sheet').performAction('dismiss');
  });
}
