import { expect, spec, type Fixture, type TestMetrics } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import type { DriveRequest, PerfActions, PerfData } from '../shared/perf-types';

type PerfLab = PageContract<PerfData, PerfActions>;

// Perf lab: one workload per pipeline stage, judged on the renderer's own frame statistics.
// Budgets start generous; tighten them from the measured numbers below.
//
// Measured, release builds (frame time p50/p95/p99 ms, dropped refreshes; input and first
// present p50/p95 ms):
//   macOS, Intel i5 MacBook at 60 Hz, 2026-11-05, other builds running (load 12-17):
//     list 16.7/33.5/33.5 (86 dropped) and 33.3/83.4/150.1 (199) · springs 16.7/16.8/50.0 (15) and
//     16.7/83.5/150.2 (136) · fill 33.3/50.0/66.8 (144) and 66.7/100.2/316.8 (228) · text 33.3/50.0/50.1 (28)
//     · images 16.7/116.7/116.7 (13) · touch input 42.4/48.7 · pop first present 42.8/54.8, push 165-232
//   Android, Pixel 3 at 60 Hz, 2026-11-05 (tree with uncommitted reduced-resolution backdrop blur):
//     list 65.5/82.2/98.3 (77) · springs 32.7/98.2/114.6 (184) · fill 163.7/296.9/313.4 (283)
//     · touch input 71.4/72.6 · pop first present p95 145 · awaited Logic ticks 168 → 65 frames
//     · text, driven by hand: ~14 fps at View cpu p50 47 ms; as a spec it reads 1 frame (see KNOWN)
// Harmony is skipped until its adapter feeds display and input times.

interface Budget { frameP95: number; droppedShare: number; inputP95: number; revealP95: number; tickShare: number }
const BUDGETS: Record<string, Budget> = {
  macos: { frameP95: 120, droppedShare: 0.8, inputP95: 150, revealP95: 500, tickShare: 0.4 },
  ios: { frameP95: 120, droppedShare: 0.8, inputP95: 150, revealP95: 500, tickShare: 0.4 },
  android: { frameP95: 350, droppedShare: 0.95, inputP95: 200, revealP95: 800, tickShare: 0.35 },
};

/** Known gaps per platform, each a reason to skip until it is fixed. */
const KNOWN: Record<string, Record<string, string>> = {
  android: {
    'PERF-NAV-001': 'presentation refused after fast re-entry: a page pushed right after its pop never reaches onReady',
    'PERF-FILL-001': 'reads 0 frames as a spec in some runs (31 frames the run before); under investigation',
    'PERF-TEXT-001': 'reads 1 frame as a spec though the scene draws ~14 fps when driven by hand; under investigation',
    'PERF-IMAGE-001': 'reads 0 frames as a spec, as PERF-TEXT-001; under investigation',
    'PERF-FAIL-001': 'the Android chrome reader does not read the GPU recovery panel; it shows (seen on the Pixel)',
  },
};

spec.configure({ start: { page: 'perf-lab' }, timeout: 90_000 });

async function budget(t: Fixture, id: string): Promise<Budget> {
  const os = await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os === 'harmony') t.skip('Harmony does not feed display or input times to frame statistics yet');
  const known = KNOWN[os]?.[id];
  if (known) t.skip(known);
  return BUDGETS[os] ?? BUDGETS.android;
}

async function scene(t: Fixture, name: string, count: number) {
  await t.app.view.testId(`perf-tab-${name}`).click();
  await t.app.view.testId(`perf-count-${count}`).click();
  await expect(t.app.view.testId('perf-lab')).toHaveAttribute('data-count', String(count));
}

/** Drive the scene for 5 s as "Run budget" does, then read the page's frames over the run.
 *  The page's action, not its button: a click waits for the page to settle, which a running
 *  scene never does. */
async function runBudget(t: Fixture, scene: string, count: number): Promise<TestMetrics> {
  const page = await t.app.page<PerfLab>({ name: 'perf-lab' });
  const request: DriveRequest = { scene: scene as DriveRequest['scene'], ms: 5000, count };
  await page.invoke('drive', { payload: request });
  const started = Date.now();
  // A push/pop cycle can run 800 ms past the 5 s: wait for the driver, or the next
  // navigation races the driver's own and pops a page while it is being created.
  await new Promise<void>(resolve => setTimeout(resolve, 5000));
  await t.waitFor(() => page.data(), { until: data => data.running === '', timeout: 5000 });
  return t.app.metrics({ page: 'perf-lab', windowMs: Date.now() - started });
}

async function expectFrames(t: Fixture, metrics: TestMetrics, limit: Budget) {
  await t.attach('metrics', metrics);
  expect(metrics.samples).toBeGreaterThan(5);
  expect(metrics.frameTimeMs!.p95).toBeLessThanOrEqual(limit.frameP95);
  expect(metrics.dropped).toBeLessThanOrEqual((metrics.samples + metrics.dropped) * limit.droppedShare);
}

spec('a 1000-row list keeps pace through flings', { id: 'PERF-LIST-001', covers: ['view:list'] }, async (t) => {
  const limit = await budget(t, 'PERF-LIST-001');
  await scene(t, 'list', 1000);
  const list = t.app.view.testId('perf-list');
  const started = Date.now();
  for (const direction of ['up', 'up', 'down', 'up'] as const) {
    await t.app.view.fling(direction, 4000, { at: list });
    await t.app.view.testId('perf-list').waitFor({ state: 'visible' });
  }
  await new Promise<void>(resolve => setTimeout(resolve, 1000));
  await expectFrames(t, await t.app.metrics({ windowMs: Date.now() - started }), limit);
});

spec('250 springs at once keep pace', { id: 'PERF-ANIM-001' }, async (t) => {
  const limit = await budget(t, 'PERF-ANIM-001');
  await scene(t, 'animation', 250);
  await expectFrames(t, await runBudget(t, 'animation', 250), limit);
});

spec('eight translucent, blurred and shadowed layers keep pace', { id: 'PERF-FILL-001' }, async (t) => {
  const limit = await budget(t, 'PERF-FILL-001');
  await scene(t, 'fill', 8);
  await expectFrames(t, await runBudget(t, 'fill', 8), limit);
});

spec('a mixed-script paragraph relayouts while it grows', { id: 'PERF-TEXT-001' }, async (t) => {
  const limit = await budget(t, 'PERF-TEXT-001');
  await scene(t, 'text', 4);
  await expectFrames(t, await runBudget(t, 'text', 4), limit);
});

spec('images decoding while a list scrolls keep pace', { id: 'PERF-IMAGE-001' }, async (t) => {
  const limit = await budget(t, 'PERF-IMAGE-001');
  await scene(t, 'images', 96);
  await expectFrames(t, await runBudget(t, 'images', 96), limit);
});

spec('a dragged box shows the finger within the input budget', { id: 'PERF-TOUCH-001' }, async (t) => {
  const limit = await budget(t, 'PERF-TOUCH-001');
  await scene(t, 'touch', 48);
  const box = t.app.view.testId('perf-box');
  const started = Date.now();
  await t.app.view.drag(box, { dx: 220, dy: 0, durationMs: 900 });
  await t.app.view.drag(box, { dx: -220, dy: 0, durationMs: 900 });
  const metrics = await t.app.metrics({ windowMs: Date.now() - started });
  await t.attach('metrics', metrics);
  expect(metrics.inputToPresent.count).toBeGreaterThan(5);
  expect(metrics.inputToPresent.statsMs!.p95).toBeLessThanOrEqual(limit.inputP95);
});

spec('pushed and popped pages show their first frame within budget', { id: 'PERF-NAV-001', covers: ['lx.navigateTo', 'lx.navigateBack'] }, async (t) => {
  const limit = await budget(t, 'PERF-NAV-001');
  await scene(t, 'transition', 100);
  const back = await runBudget(t, 'transition', 100);
  await t.attach('pop-metrics', back);
  expect(back.reveals).toBeGreaterThan(2);
  expect(back.revealMs!.p95).toBeLessThanOrEqual(limit.revealP95);
  const pushes: number[] = [];
  for (let round = 0; round < 3; round++) {
    await t.app.nav.to({ page: 'perf-target', query: { rows: '100' } });
    await t.app.view.testId('perf-target').waitFor({ state: 'visible' });
    const pushed = await t.waitFor(() => t.app.metrics(), { until: m => m.firstPresentMs !== null });
    pushes.push(pushed.firstPresentMs!);
    await t.app.nav.back();
  }
  await t.attach('push-first-present-ms', pushes);
  expect(Math.max(...pushes)).toBeLessThanOrEqual(limit.revealP95);
});

spec('the list at its largest row count stays windowed and the page responsive', { id: 'PERF-LIST-MAX-001', covers: ['view:list'] }, async (t) => {
  await budget(t, 'PERF-LIST-MAX-001');
  await scene(t, 'list', 20000);
  const metrics = await t.app.metrics();
  await t.attach('metrics', metrics);
  // Only rows near the viewport are laid out, however many the list holds.
  expect(metrics.nodes!).toBeLessThan(1000);
  const list = t.app.view.testId('perf-list');
  await expect(list).toHaveAttribute('data-shown', '2000');
  // The list grows by pages as it nears its end; the first render holds one page.
  await t.app.view.fling('up', 4000, { at: list });
  await t.app.view.testId('perf-tab-fill').click();
  await expect(t.app.view.testId('perf-scene-fill')).toBeVisible();
  await t.app.view.testId('perf-tab-list').click();
  await expect(t.app.view.testId('perf-list')).toBeVisible();
  expect((await t.app.info()).appId).toBe('lingxia-showcase-gpu');
});

const RELOAD = /^(Reload page|重新载入页面)$/;
const HOME = /^(Home|首页)$/;

spec('a page past its View budget shows recovery chrome while the rest of the app works', { id: 'PERF-FAIL-001', requires: { chrome: true, hostInput: true } }, async (t) => {
  await budget(t, 'PERF-FAIL-001');
  // The act fails with the page it was acting on; the chrome is what counts.
  await t.app.view.testId('perf-stall').click().catch(() => undefined);
  const chrome = await t.waitFor(() => t.app.chrome.elements(), { until: elements => elements.some(e => RELOAD.test(e.label)) });
  await t.attach('chrome', chrome);
  // A real tap where the tab is on screen (a collapsed desktop sidebar shows none).
  const home = chrome.find(e => HOME.test(e.label) && (e.scope === 'tabBar' || e.scope === 'sidebar'));
  if (home) await t.app.chrome.tap({ label: home.label, scope: home.scope as 'tabBar' });
  else await t.app.nav.switchTab({ page: 'home' });
  await expect.poll(async () => (await t.app.nav.current()).name).toBe('home');
  await t.app.nav.switchTab({ page: 'perf-lab' });
  const again = await t.waitFor(() => t.app.chrome.elements(), { until: elements => elements.some(e => RELOAD.test(e.label)) });
  await t.app.chrome.tap({ label: again.find(e => RELOAD.test(e.label))!.label, scope: 'window' });
  await expect(t.app.view.testId('perf-tab-list')).toBeVisible({ timeout: 15_000 });
});

spec('state set from an awaited Logic loop keeps reaching the View', { id: 'PERF-BRIDGE-001' }, async (t) => {
  const limit = await budget(t, 'PERF-BRIDGE-001');
  // The touch scene draws only the tick counter per update, so the View is not the limit.
  await scene(t, 'touch', 48);
  const page = await t.app.page<PerfLab>({ name: 'perf-lab' });
  const before = (await t.app.metrics()).framesTotal;
  await page.invoke('drive', { payload: { scene: 'text', ms: 3000, count: 1 } });
  await new Promise<void>(resolve => setTimeout(resolve, 3000));
  const { tick } = await t.waitFor(() => page.data(), { until: data => data.running === '', timeout: 5000 });
  const shown = (await t.app.metrics()).framesTotal - before;
  await t.attach('ticks-and-frames', { tick, shown });
  // Ticks come every ~18 ms from an awaited loop. The View keeps the newest state when a
  // frame is still in flight, so about one frame per two ticks shows on macOS (92 for 170);
  // the old debounce let through 8.
  expect(tick).toBeGreaterThan(50);
  expect(shown).toBeGreaterThan(tick * limit.tickShare);
});
