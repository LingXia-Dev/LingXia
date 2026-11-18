import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo.js';
import { decodePng } from './png.js';

spec('embedded webview loads packaged HTML, syncs #hash without reload, enforces mailbox and source policies', { start: { page: 'spec-webview' } }, async (t) => {
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

spec('embedded webview composites beneath View overlays, suppresses semantics under modal dialog, and hands off to a 70% dialog sheet without reload', { start: { page: 'spec-webview' } }, async (t) => {
  const view = t.app.view;

  await expect(view.testId('wv-state-load')).toContainText('title=Embed Demo|origin=lxapp|iframeClean=true', { timeout: 8_000 });
  const epochBefore = /^epoch=([^|]+)\|/.exec(await view.testId('wv-state-epoch').textContent())?.[1] ?? '';
  expect(epochBefore.length).toBeGreaterThan(3);

  // Composited screenshot shows the emerald HTML surface inside `<webview>` and the View FAB on top.
  const wvBox = await view.testId('spec-webview-frame').boundingBox();
  const fabBox = await view.testId('spec-webview-fab').boundingBox();
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
  expect(sheetBox.height).toBeGreaterThan(viewport.height * 0.65);
  expect(sheetBox.height).toBeLessThan(viewport.height * 0.75);

  await view.testId('wv-sheet-close').click();
  await expect(view.testId('wv-sheet')).not.toBeAttached();
  await expect(view.testId('wv-state-epoch')).toContainText(`epoch=${epochBefore}|`, { timeout: 3_000 });
  await expect(view.testId('wv-state-epoch')).toContainText('|sheet=false', { timeout: 3_000 });
});

spec('view-media webview demo opens a 70% sheet via floating button and exits via Close, backdrop, or Escape without reload', { start: openDemo('webview') }, async (t) => {
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
