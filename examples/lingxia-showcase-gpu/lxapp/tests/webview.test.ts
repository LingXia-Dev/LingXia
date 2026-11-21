import { expect, test, type Fixture } from '@lingxia/test';
import { openDemo } from './open-demo.js';
import { decodePng } from './png.js';

type Locator = ReturnType<Fixture['app']['view']['testId']>;

/** Scrolls `target` wholly on screen and returns its box: a gesture or a pixel probe off screen misses on a short screen. */
async function onScreen(t: Fixture, target: Locator) {
  await target.scrollIntoView();
  const [box, viewport] = await Promise.all([target.boundingBox(), t.app.view.viewport()]);
  expect(box.y, 'top on screen').toBeGreaterThanOrEqual(viewport.top - 1);
  expect(box.y + box.height, 'bottom on screen').toBeLessThanOrEqual(viewport.top + viewport.height + 1);
  return box;
}

test('embedded webview loads packaged HTML, syncs #hash without reload, enforces mailbox and source policies', { start: { page: 'spec-webview' } }, async (t) => {
  const view = t.app.view;

  await expect(view.testId('wv-state-load')).toContainText('title=Embed Demo|origin=lxapp|iframeClean=true', { timeout: 8_000 });
  const initialEpochLine = await view.testId('wv-state-epoch').textContent();
  const initialEpoch = /^epoch=([^|]+)\|/.exec(initialEpochLine)?.[1] ?? '';
  expect(initialEpoch.length).toBeGreaterThan(3);

  // Fragment change updates location.hash in-place without reloading the document.
  await view.testId('wv-btn-hash').click();
  await expect(view.testId('wv-state-epoch')).toHaveText(`epoch=${initialEpoch}|hash=#section-2|sheet=false`, { timeout: 5_000 });

  // Unqueued messages coalesce per frame; queued messages arrive in FIFO order.
  await view.testId('wv-btn-coalesce').click();
  await expect.poll(async () => {
    const line = await view.testId('wv-state-msg').textContent();
    const raw = /^coalesced=([^|]*)\|/.exec(line)?.[1] ?? '';
    const parts = raw ? raw.split(',') : [];
    return parts.length > 0 && parts.length < 20 && parts[parts.length - 1] === '20';
  }, { timeout: 5_000 }).toBe(true);

  await view.testId('wv-btn-queue').click();
  await expect(view.testId('wv-state-msg')).toContainText('queued=1,2,3,4|', { timeout: 5_000 });

  // >64 KiB throws RangeError synchronously in the guest document.
  await view.testId('wv-btn-oversize').click();
  await expect(view.testId('wv-state-msg')).toContainText('oversize=RangeError|', { timeout: 5_000 });

  // >64 queued messages in one burst drops the oldest and reports WEBVIEW_MESSAGE_DROPPED.
  await view.testId('wv-btn-overflow').click();
  await expect(view.testId('wv-state-msg')).toContainText('|flood=400', { timeout: 5_000 });
  await expect(view.testId('wv-state-err')).toContainText('err=WEBVIEW_MESSAGE_DROPPED', { timeout: 5_000 });

  // Off-origin top-level navigation is blocked and reported via onBlocked while keeping the document alive.
  await view.testId('wv-btn-block').click();
  await expect(view.testId('wv-state-err')).toContainText('blocked=https://intranet/escape|', { timeout: 5_000 });
  await expect(view.testId('wv-state-epoch')).toContainText(`epoch=${initialEpoch}|`, { timeout: 3_000 });

  // Disallowed scheme -> WEBVIEW_SOURCE; ungranted HTTPS host -> WEBVIEW_NOT_GRANTED.
  await view.testId('wv-btn-bad-src').click();
  await expect(view.testId('wv-state-err')).toContainText('err=WEBVIEW_SOURCE', { timeout: 5_000 });

  await view.testId('wv-btn-ungranted').click();
  await expect(view.testId('wv-state-err')).toContainText('err=WEBVIEW_NOT_GRANTED', { timeout: 5_000 });
});

test('embedded webview composites beneath View overlays, suppresses semantics under modal dialog, and hands off to a 70% dialog sheet without reload', { start: { page: 'spec-webview' } }, async (t) => {
  const view = t.app.view;

  await expect(view.testId('wv-state-load')).toContainText('title=Embed Demo|origin=lxapp|iframeClean=true', { timeout: 8_000 });
  const epochBefore = /^epoch=([^|]+)\|/.exec(await view.testId('wv-state-epoch').textContent())?.[1] ?? '';
  expect(epochBefore.length).toBeGreaterThan(3);

  // Composited screenshot shows the emerald HTML surface inside `<webview>` and the View FAB on top.
  const wvBox = await onScreen(t, view.testId('spec-webview-frame'));
  const fabBox = await onScreen(t, view.testId('spec-webview-fab'));
  const vp = await view.viewport();
  await expect.poll(async () => {
    const raw = await view.screenshot();
    const scale = raw.width / vp.width;
    const pts: [number, number][] = [
      [Math.round((wvBox.x + 24) * scale), Math.round((wvBox.y + wvBox.height - 20) * scale)],
      [Math.round((fabBox.x + 6) * scale), Math.round((fabBox.y + fabBox.height / 2) * scale)],
    ];
    const shot = await decodePng(
      raw.base64,
      () => Math.max(pts[0][1], pts[1][1]) + 2,
      () => Math.max(pts[0][0], pts[1][0]) + 2,
    );
    const htmlPx = shot.at(pts[0][0], pts[0][1]);
    const fabPx = shot.at(pts[1][0], pts[1][1]);
    return htmlPx[1] > htmlPx[0] + 40 && htmlPx[1] > 100 && fabPx[0] > fabPx[1] + 40;
  }, { timeout: 5_000 }).toBe(true);

  // Tapping the View FAB over `<webview>` hits View, never the HTML page underneath.
  await view.swipe({
    from: [fabBox.x + fabBox.width / 2, fabBox.y + fabBox.height / 2],
    to: [fabBox.x + fabBox.width / 2, fabBox.y + fabBox.height / 2],
    durationMs: 60,
    hold: 0,
  });
  await expect(view.testId('wv-state-taps')).toHaveText('html=0|fab=1', { timeout: 3_000 });

  // Modal `<dialog>` without the web handle hides the inline `<webview>` from accessibility until closed.
  expect(await view.css('[role="web-view"][aria-label="Embedded demo document"]').count()).toBe(1);
  await view.testId('wv-btn-dialog').click();
  await expect(view.testId('wv-modal')).toBeVisible();
  expect(await view.css('[role="web-view"][aria-label="Embedded demo document"]').count()).toBe(0);
  await view.testId('wv-modal-close').click();
  await expect(view.testId('wv-modal')).not.toBeVisible();
  expect(await view.css('[role="web-view"][aria-label="Embedded demo document"]').count()).toBe(1);

  // `useWebView` hands the live WebView into a 70%-height `<dialog>` sheet with no runtime close button and zero reload.
  await view.testId('wv-btn-sheet').click();
  await expect(view.testId('wv-sheet')).toBeVisible({ timeout: 5_000 });
  expect(await view.testId('lx-webview-close').count()).toBe(0);
  const viewport = await view.viewport();
  const sheetBox = await view.testId('wv-sheet').boundingBox();
  expect(sheetBox.width).toBeGreaterThanOrEqual(viewport.width - 2);
  // The sheet docks to the bottom edge, so its 70% is of the area below the top bars.
  const top = Number(/top=([\d.]+)/.exec(await view.testId('wv-state-inset').textContent())?.[1]);
  const area = viewport.height - top;
  expect(sheetBox.height).toBeGreaterThan(area * 0.65);
  expect(sheetBox.height).toBeLessThan(area * 0.75);

  await view.testId('wv-sheet-close').click();
  await expect(view.testId('wv-sheet')).not.toBeAttached();
  await expect(view.testId('wv-state-epoch')).toContainText(`epoch=${epochBefore}|`, { timeout: 3_000 });
  await expect(view.testId('wv-state-epoch')).toContainText('|sheet=false', { timeout: 3_000 });
});

test('view-media webview demo opens a 70% sheet via floating button and exits via Close, backdrop, or Escape without reload', { start: openDemo('webview') }, async (t) => {
  const view = t.app.view;
  const readout = view.testId('lab-webview-state');

  // Navigate into view-media, back out, and immediately back in: guards against retiring
  // native WebView teardown clobbering the replacement controller on the same page path.
  await t.app.nav.relaunch({ page: 'view' });
  await t.app.nav.to(openDemo('webview'));
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash= taps=0 sheet=false', { timeout: 8_000 });
  await t.app.nav.back();
  await t.app.nav.to(openDemo('webview'));
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash= taps=0 sheet=false', { timeout: 8_000 });

  // Clicking #part-2 / #part-1 drives in-place hashchange inside the HTML document and round-trips via postMessage.
  await view.testId('lab-webview-hash').click();
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash=#part-2 taps=0 sheet=false', { timeout: 5_000 });
  await view.testId('lab-webview-hash').click();
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash=#part-1 taps=0 sheet=false', { timeout: 5_000 });

  // 1) Floating expand button over `<webview>` opens the 70% `<dialog>` sheet -> Close button dismisses it.
  await view.testId('lab-webview-expand').click();
  await expect(view.testId('lab-webview-sheet')).toBeVisible({ timeout: 5_000 });
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash=#part-1 taps=0 sheet=true', { timeout: 3_000 });
  expect(await view.testId('lx-webview-close').count()).toBe(0);
  await view.testId('lab-webview-sheet-close').click();
  await expect(view.testId('lab-webview-sheet')).not.toBeAttached();
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash=#part-1 taps=0 sheet=false', { timeout: 3_000 });

  // 2) Backdrop tap (`closedby="any"`) in the top 30% scrim dismisses the 70% sheet.
  await view.testId('lab-webview-expand').click();
  await expect(view.testId('lab-webview-sheet')).toBeVisible({ timeout: 5_000 });
  await view.testId('lab-webview-sheet-backdrop').click({ force: true });
  await expect(view.testId('lab-webview-sheet')).not.toBeAttached({ timeout: 5_000 });
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash=#part-1 taps=0 sheet=false', { timeout: 3_000 });

  // 3) Escape / Back close request dismisses the 70% sheet without popping `view-media`.
  await view.testId('lab-webview-expand').click();
  const closeBtn = view.testId('lab-webview-sheet-close');
  await expect(closeBtn).toBeVisible({ timeout: 5_000 });
  await closeBtn.press('Escape');
  await expect(view.testId('lab-webview-sheet')).not.toBeAttached({ timeout: 5_000 });
  await expect(readout).toHaveAttribute('data-state', 'title=Embed Demo hash=#part-1 taps=0 sheet=false', { timeout: 3_000 });
  expect((await t.app.nav.current()).path).toContain('view-media');
});

// `handlesDrags`: two copies of a page that cannot scroll. A drag on the plain one belongs to the
// screen; the other keeps every drag that starts on it. On iOS the runner injects its drag into the
// View page, never into the native web view, so both legs there are checked by hand on a device.
async function webDrags(t: Fixture) {
  const os = await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os === 'ios') t.skip('iOS: the runner\'s drag never reaches the native web view; check by hand on a device');
  const view = t.app.view;
  // A press on Reset brings both pages on screen above it.
  await view.testId('lab-web-drags-reset').click();
  const state = view.testId('lab-web-drags-state');
  await expect(state).toHaveAttribute('data-state', /^loaded=2 /, { timeout: 8_000 });
  return { view, state };
}

/** Skips when the runner's drag went to the View page; `byHand` says what a device agent checks instead. */
const skipWhenInjected = (t: Fixture, trace: { delivered: string }, byHand: string) => {
  if (trace.delivered === 'view') t.skip(`the drag was injected into the View page and never reached the native web view. By hand: ${byHand}`);
};

test('a drag on a web page that does not keep its drags scrolls the screen', { covers: ['view:webview', 'view:scroll'], start: openDemo('webDrags'), requires: { input: ['touch'] } }, async (t) => {
  const { view, state } = await webDrags(t);
  const frame = view.testId('lab-web-drags-plain');
  const before = await onScreen(t, frame);
  const from: [number, number] = [before.x + before.width / 2, before.y + before.height - 12];
  skipWhenInjected(t, await view.swipe({ from, to: [from[0], from[1] - 140], durationMs: 400 }),
    'on the View catalogue\'s web-drags demo, press Reset, then drag up from the bottom of the first page: the screen scrolls, and lab-web-drags-state reads plain=none or a plain pin ending "cancelled", never "done".');
  await t.waitFor(() => frame.boundingBox(), { until: now => now.y < before.y - 40 });
  // The screen took the drag: the page saw nothing, or a cancel, never a completed drag.
  await expect(state).toHaveAttribute('data-state', /plain=(none|\d+,\d+ moves=\d+ cancelled) /, { timeout: 3_000 });
});

test('a web page with handlesDrags keeps a drag that starts on it: its pin follows and the screen stays', { covers: ['view:webview'], start: openDemo('webDrags'), requires: { input: ['touch'] } }, async (t) => {
  const { view, state } = await webDrags(t);
  const frame = view.testId('lab-web-drags-own');
  const before = await onScreen(t, frame);
  // Mostly upward, which would scroll the screen if the web page did not keep it.
  const from: [number, number] = [before.x + 40, before.y + before.height - 16];
  const to: [number, number] = [before.x + 140, before.y + 20];
  skipWhenInjected(t, await view.swipe({ from, to, durationMs: 500 }),
    'on the web-drags demo, press Reset, then drag up and right across the second page: its pin follows the finger, the screen does not move, and lab-web-drags-state reads own=… moves=N done.');
  await expect(state).toHaveAttribute('data-state', /own=\d+,\d+ moves=[1-9]\d* done/, { timeout: 5_000 });
  const [, x, y] = /own=(\d+),(\d+)/.exec((await state.getAttribute('data-state')) ?? '') ?? [];
  // The pin is where the finger let go, in the web page's own pixels.
  expect(Math.abs(Number(x) - 140)).toBeLessThan(16);
  expect(Math.abs(Number(y) - 20)).toBeLessThan(16);
  const after = await frame.boundingBox();
  expect(Math.abs(after.y - before.y)).toBeLessThan(2);

  // A second, equal drag counts its own moves, not the first one's as well.
  const moves = async () => Number(/own=\d+,\d+ moves=(\d+)/.exec((await state.getAttribute('data-state')) ?? '')?.[1]);
  const first = await moves();
  await view.testId('lab-web-drags-reset').click();
  await expect(state).toHaveAttribute('data-state', / own=none$/, { timeout: 3_000 });
  await view.swipe({ from, to, durationMs: 500 });
  await expect(state).toHaveAttribute('data-state', /own=\d+,\d+ moves=[1-9]\d* done/, { timeout: 5_000 });
  expect(await moves(), `the second drag restarts the count (the first had ${first})`).toBeLessThan(first * 1.5);
});

// The page reads only `prefers-color-scheme`: no hash, no message from the View page.
test('an embedded web page follows the View page\'s appearance live, without the hash or a reload', { covers: ['view:webview', 'view:environment'], start: openDemo('webScheme') }, async (t) => {
  const state = t.app.view.testId('lab-web-scheme-state');
  const read = async () => (await state.getAttribute('data-state')) ?? '';
  for (const theme of ['light', 'dark', 'light'] as const) {
    await t.app.view.environment({ theme });
    await expect(state).toHaveAttribute('data-state', new RegExp(`^loads=1 scheme=${theme} `), { timeout: 8_000 });
  }
  // Each switch reached the running page as a change, and its own colours moved with it.
  expect(Number(/changes=(\d+)/.exec(await read())?.[1]), 'two live changes after the first read').toBeGreaterThanOrEqual(2);
  await t.app.view.environment({ theme: 'dark' });
  await expect(state).toHaveAttribute('data-state', /scheme=dark .*background=rgb\(30, 27, 75\)/, { timeout: 8_000 });
});

// Scrolling hands off at the edge: drags inside move the web page until its last row, then the
// next drag scrolls the screen. Only a real touch reaches the native web view.
test('a drag inside a scrollable web page scrolls it to its edge, then the screen takes the next drag', { covers: ['view:webview', 'view:scroll'], start: openDemo('webScroll'), requires: { input: ['touch'] } }, async (t) => {
  const os = await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os === 'ios') t.skip('iOS: the runner\'s drag never reaches the native web view; check by hand on a device');
  const view = t.app.view;
  const frame = view.testId('lab-web-scroll-frame');
  const state = view.testId('lab-web-scroll-state');
  await expect(state).toHaveAttribute('data-state', /^loads=1 top=0 end=false$/, { timeout: 8_000 });
  const box = await onScreen(t, frame);
  const drag = async () => {
    const from: [number, number] = [box.x + box.width / 2, box.y + box.height - 16];
    skipWhenInjected(t, await view.swipe({ from, to: [from[0], box.y + 16], durationMs: 350 }),
      'on the web-scroll demo, with the frame wholly on screen, drag up inside it until its last row shows (the screen stays put), then drag up once more there: the screen scrolls.');
  };
  // Thirty rows of 44 px in a 160 px box: drags reach the end (how many depends on the screen), and the screen stays put meanwhile.
  for (let i = 0; i < 20 && !/end=true/.test((await state.getAttribute('data-state')) ?? ''); i += 1) {
    await drag();
    const now = await frame.boundingBox();
    expect(Math.abs(now.y - box.y), 'the screen stays while the web page scrolls').toBeLessThan(2);
  }
  await expect(state).toHaveAttribute('data-state', /end=true$/, { timeout: 3_000 });
  await drag();
  await t.waitFor(() => frame.boundingBox(), { until: now => now.y < box.y - 40 });
});

// A web view in a fading box draws its picture, which fades with the box; solid again, it is the
// live page. The page is solid orange, so one pixel of it tells solid, faint and empty apart.
test('a web page in a fading box keeps showing while it fades and returns', { covers: ['view:webview'], start: openDemo('webMotion'), timeout: 60_000 }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-web-motion-state');
  await expect(state).toHaveAttribute('data-state', /loads=1 faded=false/, { timeout: 8_000 });
  const box = await onScreen(t, view.testId('lab-web-motion-box'));
  const vp = await view.viewport();
  const look = (px: number[]) => {
    const tint = px[0] - px[2];
    return tint > 180 ? 'solid' : tint > 40 ? 'faint' : 'empty';
  };
  const probe = async () => {
    const raw = await view.screenshot();
    const scale = raw.width / vp.width;
    // A quarter down, clear of the page's centred label.
    const at = [Math.round((box.x + box.width / 2) * scale), Math.round((box.y + box.height / 4) * scale)];
    const shot = await decodePng(raw.base64, () => at[1] + 2);
    return look(shot.at(at[0], at[1]));
  };
  // A screenshot takes seconds on a slow phone: each probe waits out the 700 ms transition first.
  const settled = { timeout: 20_000 };
  await expect.poll(probe, settled).toBe('solid');
  await view.testId('lab-web-motion-toggle').click();
  await expect(state).toHaveAttribute('data-state', /faded=true/);
  await new Promise(resolve => setTimeout(resolve, 1_000));
  await expect.poll(probe, settled).toBe('faint');
  await view.testId('lab-web-motion-toggle').click();
  await expect(state).toHaveAttribute('data-state', /faded=false/);
  await new Promise(resolve => setTimeout(resolve, 1_000));
  await expect.poll(probe, settled).toBe('solid');
  expect(await state.getAttribute('data-state'), 'the web page did not reload').toMatch(/loads=1 /);
});
