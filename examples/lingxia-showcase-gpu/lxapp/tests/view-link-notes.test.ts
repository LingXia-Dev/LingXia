import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';
import { reveal } from './pixels';

type Box = { x: number; y: number; width: number; height: number };
// How far a note sits from what it is anchored to: just above or below it, overlapping it across.
const against = (note: Box, link: Box) => {
  const gap = note.y >= link.y + link.height ? note.y - (link.y + link.height) : link.y - (note.y + note.height);
  return gap > -1 && gap < 16 && note.x < link.x + link.width && note.x + note.width > link.x;
};

spec('a link mid-sentence opens its note against itself, says it is expanded, and the sentence keeps its lines', { covers: ['view:link', 'view:popover'], start: openDemo('linkNotes') }, async (t) => {
  const view = t.app.view;
  const link = view.testId('lab-notes-link');
  const note = view.testId('lab-notes-note');
  const sentence = view.testId('lab-notes-sentence');
  await expect(note).toHaveCount(0);
  await expect(link).toHaveAttribute('aria-expanded', 'false');
  await link.click();
  await expect(note).toBeVisible();
  await expect(link).toHaveAttribute('aria-expanded', 'true');
  // The open note takes no room in the line.
  const open = (await sentence.boundingBox()).height;
  expect(against(await note.boundingBox(), await link.boundingBox())).toBe(true);
  await link.press('Escape');
  await expect(note).toHaveCount(0);
  await expect(link).toHaveAttribute('aria-expanded', 'false');
  expect((await sentence.boundingBox()).height).toBe(open);
});

spec('a hint on a link shows while keyboard focus rests on it, on a link that wraps onto two lines too', { covers: ['view:link', 'view:popover', 'view:a11y'], start: openDemo('linkNotes') }, async (t) => {
  const view = t.app.view;
  const hintLink = view.testId('lab-hint-link');
  const hint = view.testId('lab-hint-note');
  const wrapLink = view.testId('lab-wrap-link');
  const wrap = view.testId('lab-wrap-note');
  await expect(hint).toHaveCount(0);
  await view.testId('lab-notes-link').press('Tab');
  await expect(hintLink).toBeFocused();
  await expect(hint).toBeVisible();
  await expect(hintLink).toHaveAttribute('aria-describedby', 'lab-hint-note');
  expect(against(await hint.boundingBox(), await hintLink.boundingBox())).toBe(true);
  await hintLink.press('Tab');
  await expect(wrapLink).toBeFocused();
  await expect(wrap).toBeVisible();
  await expect(hint).toHaveCount(0);
  const two = await wrapLink.boundingBox();
  expect(two.height).toBeGreaterThan((await hintLink.boundingBox()).height * 1.5);
  expect(against(await wrap.boundingBox(), two)).toBe(true);
  await wrapLink.press('Escape');
  await expect(wrap).toHaveCount(0);
});

spec('a mouse resting on a link shows its hint, on either line of a link that wraps', { covers: ['view:link', 'view:pointer'], start: openDemo('linkNotes') }, async (t) => {
  const os = await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os !== 'macos' && os !== 'windows') t.skip('needs a mouse: desktop pointer input');
  const view = t.app.view;
  // The link does nothing on a press; this brings the demo on screen.
  await view.testId('lab-hint-link').click();
  const move = (at: [number, number]) => t.app.window.pointer.move({ at });
  const away: [number, number] = [4, 4];
  await move(away);
  const one = await view.testId('lab-hint-link').boundingBox();
  await move([Math.round(one.x + one.width / 2), Math.round(one.y + one.height / 2)]);
  await expect(view.testId('lab-hint-note')).toBeVisible({ timeout: 3_000 });
  await move(away);
  await expect(view.testId('lab-hint-note')).toHaveCount(0, { timeout: 3_000 });
  // The start of the second line, away from the first line's piece. Where the text wraps
  // follows the platform's fonts, so the link may sit lower than the first one brought on screen.
  await reveal(t, view.testId('lab-wrap-link'));
  const two = await view.testId('lab-wrap-link').boundingBox();
  await move([Math.round(two.x + 8), Math.round(two.y + two.height - 6)]);
  await expect(view.testId('lab-wrap-note')).toBeVisible({ timeout: 3_000 });
  await move(away);
  await expect(view.testId('lab-wrap-note')).toHaveCount(0, { timeout: 3_000 });
});
