import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';
import { apart, lineRows, reveal, sameHue, sample, trackFill } from './pixels';

// The control, select and composition demos, driven as a user drives them.
test('the controls take a tap, a drag and typing as a user gives them', { covers: ['view:radio', 'view:slider', 'view:textarea'], start: openDemo('controls') }, async (t) => {
  const view = t.app.view;
  const values = view.testId('lab-controls-values');
  await expect(values).toHaveAttribute('data-state', 'size=m volume=40 notes=0');

  await view.testId('lab-controls-size-l').click();
  await expect(values).toHaveAttribute('data-state', /^size=l /);
  await expect(view.testId('lab-controls-size-xl')).not.toBeEnabled();

  // Filling the notes brings the slider above them on screen for the drag.
  await view.testId('lab-controls-notes').fill('two\nlines');
  await expect(values).toHaveAttribute('data-state', /notes=9$/);
  await view.drag(view.testId('lab-controls-volume'), { dx: 200, dy: 0, durationMs: 400 });
  await expect.poll(async () => Number(((await values.getAttribute('data-state')) ?? '').match(/volume=(\d+)/)?.[1])).toBeGreaterThan(40);
});

test('kit progress reads as a progressbar with its value, and without one while indeterminate', { covers: ['view:a11y'], start: openDemo('progress') }, async (t) => {
  const view = t.app.view;
  const upload = view.getByRole('progressbar', { name: 'Upload' });
  await expect(upload).toHaveValue('40');
  await expect(view.getByRole('progressbar', { name: 'Storage' })).toHaveValue('40');
  // Indeterminate: the role without a value.
  await expect(view.getByRole('progressbar', { name: 'Syncing' })).not.toHaveValue(/./);
  await expect(view.getByRole('progressbar', { name: 'Connecting' })).not.toHaveValue(/./);
  await view.testId('lab-progress-more').click();
  await expect(upload).toHaveValue('60');
  const width = async (id: string) => (await view.testId(id).boundingBox()).width;
  await expect.poll(async () => Math.round(await width('lab-progress-bar-fill') / await width('lab-progress-bar') * 100)).toBe(60);
});

test('a slider styled by parts reads its valuetext and grows its line and thumb while held', { start: openDemo('controls') }, async (t) => {
  const view = t.app.view;
  const scrub = view.getByRole('slider', { name: 'Scrub' });
  await expect(scrub).toHaveAttribute('aria-valuetext', '0:30 of 3:00');
  await reveal(t, scrub, 0.4);
  const box = await scrub.boundingBox();
  // The touch target is the slider's box though its thumb draws nothing at rest.
  const at: [number, number] = [box.x + box.width / 2, box.y + box.height / 2];
  const rest = await lineRows(t, scrub);
  const finger = await view.touch(at, { press: true });
  await new Promise<void>(resolve => setTimeout(resolve, 400));
  const held = await lineRows(t, scrub);
  await finger.moveTo([at[0] + box.width / 6, at[1]], { durationMs: 300 });
  await new Promise<void>(resolve => setTimeout(resolve, 400));
  const dragged = await lineRows(t, scrub);
  await finger.up();
  expect(held > rest && dragged > rest ? 'grew' : `same ${JSON.stringify({ rest, held, dragged })}`).toBe('grew');
  await expect(scrub).not.toHaveAttribute('aria-valuetext', '0:30 of 3:00');
  await expect.poll(async () => lineRows(t, scrub), { timeout: 3_000 }).toBe(rest);
});

test('a switch styled by parts recolours its track when on, and a disabled switch that is on keeps its accent', { start: openDemo('controls') }, async (t) => {
  const view = t.app.view;
  const power = view.getByRole('switch', { name: 'Power' });
  await reveal(t, power);
  const off = await trackFill(t, power);
  await power.click();
  await expect.poll(async () => apart(await trackFill(t, power), off), { timeout: 3_000 }).toBe(true);
  // The accent as this theme draws it: the track Power just took. Disabled, it fades but keeps that hue.
  const accent = await trackFill(t, power);
  const locked = await trackFill(t, view.getByRole('switch', { name: 'Locked on' }));
  expect(sameHue(locked, accent) ? 'accent' : `grey ${JSON.stringify({ locked, accent })}`).toBe('accent');
});

test('a long select label stays on one line', { start: openDemo('select') }, async (t) => {
  const view = t.app.view;
  const billing = view.getByRole('combobox', { name: 'Billing' });
  expect((await billing.boundingBox()).height).toBeLessThan(56);
});

test('a ring in the surface role paints the surface it sits on, and a role="img" avatar speaks only its name', { start: openDemo('rings') }, async (t) => {
  const view = t.app.view;
  await expect(view.getByRole('img', { name: 'Ada Lovelace' })).toHaveCount(1);
  await expect(view.getByRole('img', { name: 'Alan Turing' })).toHaveCount(1);
  await expect(view.getByRole('img', { name: 'AL' })).toHaveCount(0);
  const ada = view.testId('lab-avatar-ada'), alan = view.testId('lab-avatar-alan');
  await reveal(t, alan);
  const a = await ada.boundingBox(), b = await alan.boundingBox();
  // Mid-ring left of Alan, over Ada's fill; the stage just above the avatars.
  const [ring, stage] = await sample(t, [[b.x - 1, b.y + b.height / 2], [a.x + a.width / 2, a.y - 6]]);
  expect(apart(ring, stage) ? `ring ${JSON.stringify({ ring, stage })}` : 'matches').toBe('matches');
});

test('a provided value reaches List rows under rtl and follows its changes', { covers: ['view:list'], start: openDemo('shared') }, async (t) => {
  const view = t.app.view;
  // Rows of a List below the fold are not rendered yet; a tap on the picked row reveals it and changes nothing.
  await view.testId('lab-choice-alpha').click();
  await expect(view.testId('lab-choice-alpha-state')).toHaveText('md on');
  await expect(view.testId('lab-choice-beta-state')).toHaveText('md');
  await view.testId('lab-choice-beta').click();
  // The rows inherit the list's rtl: the label sits right of the state.
  const label = await view.testId('lab-choice-beta-label').boundingBox();
  const state = await view.testId('lab-choice-beta-state').boundingBox();
  expect(label.x).toBeGreaterThan(state.x);
  await expect(view.testId('lab-choice-beta-state')).toHaveText('md on');
  await expect(view.testId('lab-choice-alpha-state')).toHaveText('md');
  await view.testId('lab-choices-size').click();
  await expect(view.testId('lab-choice-gamma-state')).toHaveText('sm');
  await expect(view.testId('lab-choice-beta-state')).toHaveText('sm on');
  // Leaving the page unmounts the demo; coming back provides the defaults afresh.
  await t.app.nav.relaunch(openDemo('shared'));
  await view.testId('lab-choice-gamma').click();
  await expect(view.testId('lab-choice-gamma-state')).toHaveText('md on');
  await expect(view.testId('lab-choice-alpha-state')).toHaveText('md');
});

test('a popover opens from a List row that names it by a useId id', { covers: ['view:popover', 'view:list'], start: openDemo('shared') }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-choice-beta').click();
  await view.testId('lab-choice-gamma-more').click();
  await expect(view.testId('lab-choices-menu')).toBeVisible();
  await expect(view.testId('lab-choices-menu')).toContainText('menu for beta');
});
