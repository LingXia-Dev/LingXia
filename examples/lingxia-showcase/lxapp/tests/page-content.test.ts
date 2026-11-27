import { expect, test, type Fixture, type PixelRegion } from '@lingxia/test';
import app from '../lxapp.json';

// Every page a user can open shows content in the window, light and dark. The picture is the host
// window's (what the window system composites), not the page's own render target: a GPU layer the
// window never shows reads fine there while the user sees a blank page.

type Shot = Awaited<ReturnType<Fixture['automation']['lxapps']['screenshot']>>;

const FIXTURES: Record<string, string> = {
  'spec-orphan': 'a fixture page for one spec, not reachable from the app',
  'spec-fault': 'a fixture page that fails on purpose to show the recovery panel',
  'spec-stall': 'a fixture page that stays empty until its spec sends the first state',
  'spec-break': 'a fixture page for one spec, not reachable from the app',
  'spec-video': 'a fixture page for one spec, not reachable from the app',
  'spec-webview': 'a fixture page for one spec, not reachable from the app',
  'nav-lab': 'a fixture page that prints navigation logs for one spec',
};

/**
 * The part of the window shot that shows the page clear of bars and host chrome, wherever the host
 * placed the page: the overlap of all its possible placements, and the shot's pixels per point. The
 * reason instead when that cannot be known.
 */
async function pageRegion(t: Fixture, shot: Shot, name: string): Promise<{ region: PixelRegion; scale: number } | string> {
  const window = await t.app.chrome.viewport().catch(() => null);
  if (!window) return 'the host does not report its window size';
  const page = await t.app.view.viewport();
  const scale = shot.width / window.width;
  if (Math.abs(shot.height / scale - window.height) > 2) {
    return `the window shot (${shot.width}x${shot.height}) is not the reported window (${window.width}x${window.height} points)`;
  }
  if (page.width > window.width + 1 || page.height > window.height + 1) {
    return `the page (${page.width}x${page.height}) is larger than the window (${window.width}x${window.height}): a device frame scales it`;
  }
  const content = (await t.app.metrics({ page: name })).content;
  if (!content) return 'the host reports no content area for the page';
  const { area } = content;
  // The page sits anywhere from the window's origin to its far edge: keep what every placement shows.
  const left = Math.ceil((Math.max(0, window.width - page.width) + area.left) * scale);
  const top = Math.ceil((Math.max(0, window.height - page.height) + area.top) * scale);
  const right = Math.floor(Math.min(window.width, area.left + area.width) * scale);
  const bottom = Math.floor(Math.min(window.height, area.top + area.height) * scale);
  if (right - left < area.width * scale / 4 || bottom - top < area.height * scale / 4) {
    return `the page (${page.width}x${page.height}) covers too little of the window (${window.width}x${window.height}) to place it without its frame`;
  }
  return { region: { x: left, y: top, width: right - left, height: bottom - top }, scale };
}

/**
 * A window shot that holds still: the same picture twice in a row, or the last after 5 s (video
 * and spinners never hold). It differs from `previous`, the other appearance, once that reaches
 * the window.
 */
async function settledShot(t: Fixture, previous?: string): Promise<Shot> {
  const deadline = Date.now() + 5_000;
  let last = '';
  for (;;) {
    const shot = await t.automation.lxapps.screenshot();
    if ((shot.base64 === last && shot.base64 !== previous) || Date.now() > deadline) return shot;
    last = shot.base64;
    await new Promise<void>(resolve => setTimeout(resolve, 200));
  }
}

for (const { name } of app.pages) {
  const fixture = FIXTURES[name];
  test(`page ${name} shows content in the window, light and dark`, {
    id: `PAGE-CONTENT-${name}`, ...(fixture ? {} : { start: { page: name } }),
  }, async (t) => {
    if (fixture) return t.skip(fixture);
    let probe: Shot;
    try {
      probe = await t.automation.lxapps.screenshot();
    } catch (error) {
      return t.skip(`the host has no window screenshot: ${String(error)}`);
    }
    await t.waitFor(() => t.app.metrics({ page: name }), { until: m => m.firstPresentAt !== null, timeout: 20_000 });
    const placed = await pageRegion(t, probe, name);
    if (typeof placed === 'string') return t.skip(placed);

    const preference = await t.app.logic.eval(({ lx }) => lx.host.appearancePreference.get() ?? null);
    if (preference !== null) t.defer(() => t.app.logic.eval(({ lx }, look) => lx.host.appearancePreference.set(look), preference));
    let previous: string | undefined;
    for (const look of preference === null ? ['as set'] : ['light', 'dark']) {
      if (preference !== null) await t.app.logic.eval(({ lx }, look) => lx.host.appearancePreference.set(look as 'light' | 'dark'), look);
      const shot = await settledShot(t, previous);
      const area = await pageRegion(t, shot, name);
      if (typeof area === 'string') throw new Error(`${look}: the page area is unknown: ${area}`);
      previous = shot.base64;
      try {
        await expect(shot, `${name}, ${look}: the page's content area in the window`).toShowContent(area);
      } catch (error) {
        await t.attach(`window-${name}-${look}.png`, { mimeType: 'image/png', base64: shot.base64 });
        throw error;
      }
    }
  });
}
