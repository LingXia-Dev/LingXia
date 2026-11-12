import { expect, spec } from '@lingxia/test';
import { skipWhenCovered } from './covered.js';
import { decodePng } from './png.js';

// GPU-only fixtures read through role and accessible name, the way a screen reader does.
spec('view lab answers locators by role and name', { start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Input' }).click();
  await expect(view.getByRole('heading', { name: 'Monthly bills' })).toBeVisible();
  await expect(view.getByRole('listitem')).toHaveCount(3);
  const status = view.getByRole('status');
  await expect(status).toHaveText('Not saved yet');
  await view.getByRole('button', { name: 'Save' }).click();
  await expect(status).toHaveText('Saved 1 times');
});

spec('view lab editors take text through their accessible names', { covers: ['view:input'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.getByRole('button', { name: 'Input' }).click();
  // Below the fold: the locator reveals the editor before it types.
  await view.getByRole('textbox', { name: 'Amount' }).fill('12');
  await view.getByRole('textbox', { name: 'One-time code' }).fill('123456');
  await expect(view.testId('lab-input-values')).toContainText('code=123456 amount=12');
});

spec('a tap on a nested switch, a row button and a rich-text link reaches only that control', { covers: ['view:switch', 'view:link', 'view:button'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  const state = view.testId('lab-nested-state');
  await expect(state).toHaveText('opened=0 wifi=true removed=0');
  await view.testId('lab-nested-switch').click();
  await expect(state).toHaveText('opened=0 wifi=false removed=0');
  await view.testId('lab-nested-remove').click();
  await expect(state).toHaveText('opened=0 wifi=false removed=1');

  await view.testId('lab-tab-text').click();
  await view.testId('lab-rich-link').click();
  await expect(view.testId('lab-rich-presses')).toHaveText('Link pressed 1 times');
});

spec('the controls take a tap, a drag, typing and a fling as a user gives them', { covers: ['view:radio', 'view:slider', 'view:textarea', 'view:input', 'view:list'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  const values = view.testId('lab-controls-values');
  await expect(values).toContainText('size=m volume=40 notes=0 query= picked=');

  await view.testId('lab-controls-size-l').click();
  await expect(values).toContainText('size=l');
  await expect(view.testId('lab-controls-size-xl')).not.toBeEnabled();

  // Filling the notes brings the slider above them on screen for the drag.
  await view.testId('lab-controls-notes').fill('two\nlines');
  await expect(values).toContainText('notes=9');
  await view.drag(view.testId('lab-controls-volume'), { dx: 200, dy: 0, durationMs: 400 });
  await expect.poll(async () => Number((await values.textContent()).match(/volume=(\d+)/)?.[1])).toBeGreaterThan(40);

  const search = view.testId('lab-controls-search');
  await search.type('Row 3');
  await expect(values).toContainText('query=Row 3');
  // The keyboard can leave the list below the fold: scroll the page by the search field, as a user would.
  const area = await view.viewport();
  await view.swipe({ from: search, to: [area.left + area.width / 2, area.top + 60] });
  await expect(view.testId('lab-controls-row-2')).not.toBeAttached();
  await view.testId('lab-controls-row-31').click();
  await expect(values).toContainText('picked=Row 31');

  await search.fill('');
  // The edit can present before the View run that unfilters the list; a fling on the filtered
  // list, already at its end, would hand its distance to the page.
  await expect(values).toContainText('query= picked=Row 31');
  const list = view.testId('lab-controls-list');
  await expect(list).toBeInViewport();
  await view.fling('up', 3000, { at: list });
  await expect(view.testId('lab-controls-row-40')).toBeInViewport({ timeout: 3_000 });
});

spec('kit progress reads as a progressbar with its value, and without one while indeterminate', { covers: ['view:a11y'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
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

// The media specs play the bundled clip (`media=packaged`): the lab's own player streams public video.
spec('kit video controls start a poster video on a tap, then pause, mute and scrub it', { start: { page: 'view-lab', query: { media: 'packaged' } } }, async (t) => {
  await skipWhenCovered(t, 'view-lab');
  const view = t.app.view;
  await view.testId('lab-tab-media').click();
  const button = (name: string) => view.getByRole('button', { name });
  const time = view.testId('lab-video-controls-time');
  const seconds = async () => {
    const [, m, s] = /^(\d+):(\d+)/.exec(await time.textContent()) ?? [];
    return Number(m) * 60 + Number(s);
  };
  // Not autoplaying: it waits on its poster with the length known. The spinner beside it commits
  // the page every frame, and the player's state must reach the bar all the same.
  await expect(view.testId('lab-video-spinner')).toBeVisible();
  await expect(button('Play')).toBeVisible();
  await expect(time).toHaveText('0:00 / 0:06', { timeout: 5_000 });
  await button('Play').click();
  await expect(button('Pause')).toBeVisible();
  await expect.poll(seconds, { timeout: 5_000 }).toBeGreaterThan(0);
  await button('Pause').click();
  await expect(button('Play')).toBeVisible();
  await button('Mute').click();
  await expect(button('Unmute')).toBeVisible();
  // A drag to the far end seeks there and stays paused (at the end, the button offers Replay).
  await view.drag(view.getByRole('slider', { name: 'Seek' }), { dx: 1000, dy: 0, durationMs: 400 });
  await expect.poll(seconds, { timeout: 5_000 }).toBeGreaterThanOrEqual(5);
  await expect(button('Pause')).toHaveCount(0);
});

spec('a finger on any group tab opens its group, wrapped line included', { start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  // A real touch at each tab's centre, not a dispatch to the element: the hit test decides.
  for (const [group, shows] of [['media', 'lab-video-player'], ['data', 'lab-plots'], ['gesture', 'lab-drag'], ['swipe', 'lab-pager'], ['feedback', 'lab-progress-more']] as const) {
    const box = await view.testId(`lab-tab-${group}`).boundingBox();
    const at = [box.x + box.width / 2, box.y + box.height / 2] as const;
    await view.swipe({ from: at, to: at, durationMs: 60, hold: 0 });
    await expect(view.testId(shows).first()).toBeVisible({ timeout: 3_000 });
  }
});

spec('kit video controls show the player full screen and Escape brings it back', { start: { page: 'view-lab', query: { media: 'packaged' } } }, async (t) => {
  await skipWhenCovered(t, 'view-lab');
  const view = t.app.view;
  await view.testId('lab-tab-media').click();
  await view.testId('lab-video-controls-play').click();
  await view.testId('lab-video-controls-fullscreen').click();
  const layer = view.getByRole('dialog', { name: 'Full screen video' });
  await expect(layer).toBeVisible();
  // The same player, so the full screen bar reads its time as it plays on.
  const time = view.testId('lx-fullscreen-controls-time');
  const seconds = async () => Number(/^\d+:(\d+)/.exec(await time.textContent())?.[1] ?? NaN);
  const start = await seconds();
  await expect.poll(seconds, { timeout: 5_000 }).toBeGreaterThan(start);
  const area = await view.viewport();
  const box = await layer.boundingBox();
  expect(box.width).toBeGreaterThanOrEqual(area.width - 1);
  await layer.press('Escape');
  await expect(layer).not.toBeAttached();
  expect((await t.app.nav.current()).name).toBe('view-lab');
  await view.testId('lab-video-controls-fullscreen').click();
  await expect(layer).toBeVisible();
  await view.testId('lx-fullscreen-controls-fullscreen').click();
  await expect(layer).not.toBeAttached();
});

// A desktop window keeps the shape its user gave it; a phone turns to the picture.
spec('a landscape full-screen video turns the window and leaving gives it back', { start: { page: 'view-lab', query: { media: 'packaged' } }, requires: { input: ['touch'] } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-media').click();
  const wide = async () => { const area = await view.viewport(); return area.width > area.height; };
  // Just after launch a window the sensor turned can still report its old shape: read it settled.
  let before = await wide();
  for (let i = 0; i < 6; i++) {
    await new Promise<void>(resolve => setTimeout(resolve, 500));
    const now = await wide();
    if (now === before) break;
    before = now;
  }
  await view.testId('lab-video-controls-play').click();
  await view.testId('lab-video-controls-fullscreen').click();
  const layer = view.getByRole('dialog', { name: 'Full screen video' });
  await expect(layer).toBeVisible();
  await expect.poll(wide, { timeout: 5_000 }).toBe(true);
  await view.testId('lx-fullscreen-controls-fullscreen').click();
  await expect(layer).not.toBeAttached();
  await expect.poll(wide, { timeout: 5_000 }).toBe(before);
});

spec('a provided value reaches List rows under rtl and follows its changes', { covers: ['view:list'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-layout').click();
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
  // Leaving the group unmounts the section; coming back provides the defaults afresh.
  await view.testId('lab-tab-paint').click();
  await view.testId('lab-tab-layout').click();
  await view.testId('lab-choice-gamma').click();
  await expect(view.testId('lab-choice-gamma-state')).toHaveText('md on');
  await expect(view.testId('lab-choice-alpha-state')).toHaveText('md');
});

spec('a popover opens from a List row that names it by a useId id', { covers: ['view:popover', 'view:list'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-layout').click();
  await view.testId('lab-choice-beta').click();
  await view.testId('lab-choice-gamma-more').click();
  await expect(view.testId('lab-choices-menu')).toBeVisible();
  await expect(view.testId('lab-choices-menu')).toContainText('menu for beta');
});

spec('a field states required and invalid, names its hint and error, in a group with a tooltip and an alertdialog', { covers: ['view:a11y', 'view:input'], start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  await expect(view.getByRole('group', { name: 'Delivery' })).toHaveCount(1);
  await expect(view.getByRole('tooltip')).toHaveText('We deliver within the city');
  const zip = view.getByRole('textbox', { name: 'ZIP code' });
  await expect(zip).toHaveAttribute('aria-required', 'true');
  await expect(zip).toHaveAttribute('aria-invalid', 'false');
  const hint = (await zip.getAttribute('aria-describedby'))!;
  expect(hint.split(' ')).toHaveLength(1);
  await zip.fill('12');
  await expect(zip).toHaveAttribute('aria-invalid', 'true');
  await expect(view.testId('lab-form-error')).toHaveText('Not a ZIP code');
  const both = (await zip.getAttribute('aria-describedby'))!.split(' ');
  expect(both).toHaveLength(2);
  expect(both[0]).toBe(hint);
  await zip.fill('12345');
  await expect(zip).toHaveAttribute('aria-invalid', 'false');
  await view.getByRole('button', { name: 'Delete address' }).click();
  const confirm = view.getByRole('alertdialog', { name: 'Delete this address?' });
  await expect(confirm).toBeVisible();
  await view.getByRole('button', { name: 'Keep it' }).click();
  await expect(confirm).toHaveCount(0);
});

spec('composites take one Tab stop, arrows move between items and tabs activate on arrival', {
  start: { page: 'view-lab' },
}, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  await expect(view.getByRole('tablist', { name: 'Range' })).toHaveCount(1);
  const week = view.getByRole('tab', { name: 'Week' });
  await week.press('ArrowRight');
  await expect(view.getByRole('tab', { name: 'Month' })).toBeFocused();
  await expect(view.testId('lab-range-panel')).toHaveText('Showing the month');
  await view.getByRole('tab', { name: 'Month' }).press('ArrowRight');
  await expect(view.testId('lab-range-panel')).toHaveText('Showing the day');
  await view.testId('lab-tool-bold').press('ArrowRight');
  await expect(view.testId('lab-tool-copy')).toBeFocused();
  await view.testId('lab-tree-src').press('ArrowRight');
  await expect(view.testId('lab-tree-main')).toBeFocused();
  await view.testId('lab-tree-main').press('ArrowLeft');
  await expect(view.testId('lab-tree-src')).toBeFocused();
  await expect(view.getByRole('treeitem', { name: 'main.ts' })).toHaveCount(1);
});

spec('focus-visible paints after a key moves focus and clears after a finger tap', { start: { page: 'view-lab' } }, async (t) => {
  const view = t.app.view;
  await view.testId('lab-tab-input').click();
  const ring = view.testId('lab-focus-visible');
  const plain = await fillOf(t, ring);
  await view.testId('lab-focus-open').press('Tab');
  await expect(ring).toBeFocused();
  const keyed = await fillOf(t, ring);
  expect(apart(keyed, plain) ? 'painted' : `plain ${JSON.stringify({ keyed, plain })}`).toBe('painted');
  // A finger, not click(): automation clicks arrive as assistive input, which keeps focus visible.
  await (await view.touch(ring)).up();
  await expect.poll(async () => apart(await fillOf(t, ring), plain), { timeout: 3_000 }).toBe(false);
});

type Fixture = Parameters<Parameters<typeof spec>[2]>[0];

const apart = (a: number[], b: number[]) => a.some((value, i) => Math.abs(value - b[i]) > 40);

/** The colour just inside a control's left padding, brought on screen first. */
async function fillOf(t: Fixture, locator: ReturnType<Fixture['app']['view']['testId']>) {
  const view = t.app.view;
  let box = await locator.boundingBox();
  let viewport = await view.viewport();
  if (box.y < viewport.top || box.y + box.height > viewport.top + viewport.height) {
    const dy = box.y + box.height / 2 - (viewport.top + viewport.height / 2);
    const x = viewport.left + 12;
    const y = viewport.top + viewport.height / 2;
    await view.swipe({ from: [x, y + dy / 2], to: [x, y - dy / 2], durationMs: 600 });
    await new Promise<void>(resolve => setTimeout(resolve, 600));
    box = await locator.boundingBox();
    viewport = await view.viewport();
  }
  let scale = 1;
  const py = box.y + box.height / 2 - viewport.top;
  const shot = await decodePng((await view.screenshot()).base64, width => {
    scale = width / viewport.width;
    return Math.round(py * scale);
  });
  return shot.at(Math.round((box.x + 6 - viewport.left) * scale), Math.round(py * scale));
}
