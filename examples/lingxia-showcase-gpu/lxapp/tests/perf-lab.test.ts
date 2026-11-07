import { expect, spec, type Fixture, type TestMetrics } from '@lingxia/test';
import type { PageContract } from '@lingxia/types/page';
import type { DriveRequest, PerfActions, PerfData } from '../shared/perf-types';
import { judgeFrames, judgeInput, judgeReveal, REVEAL_P95_MS, TICK_SHARE } from '../shared/perf-judge';

type PerfLab = PageContract<PerfData, PerfActions>;

// Perf lab: one workload per pipeline stage, judged with the bar the page's "Run budget" uses
// (shared/perf-judge.ts). A scene that misses it fails here too; each spec attaches what it
// measured before it judges, so a red run still reports its numbers.

spec.configure({ start: { page: 'perf-lab' }, timeout: 90_000 });

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
  // The window opens before the call: a driver that sets state every tick keeps Logic busy,
  // and the action's reply can arrive only once the run is over.
  const started = Date.now();
  await page.invoke('drive', { payload: request });
  await new Promise<void>(resolve => setTimeout(resolve, Math.max(0, request.ms - (Date.now() - started))));
  // A push/pop cycle can run 800 ms past the 5 s: wait for the driver, or the next
  // navigation races the driver's own and pops a page while it is being created.
  await t.waitFor(() => page.data(), { until: data => data.running === '', timeout: 5000 });
  return t.app.metrics({ page: 'perf-lab', windowMs: Date.now() - started });
}

async function expectFrames(t: Fixture, metrics: TestMetrics) {
  await t.attach('metrics', metrics);
  const verdict = judgeFrames(metrics);
  await t.attach('verdict', `${verdict.line} · dropped ${metrics.dropped} of ${metrics.samples + metrics.dropped} refreshes`);
  expect(metrics.samples).toBeGreaterThan(5);
  expect(verdict.pass, verdict.line).toBe(true);
}

/** Waits until a glide has stopped: frames present only while content changes, so none for
 *  100 ms means the list holds still. A user flings again after watching it settle. */
async function glideSettled(t: Fixture, timeoutMs = 8000) {
  const until = Date.now() + timeoutMs;
  while (Date.now() < until) {
    const recent = await t.app.metrics({ windowMs: 100 });
    if (recent.samples + recent.dropped === 0) return;
    await new Promise<void>(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`the list was still gliding after ${timeoutMs} ms`);
}

spec('a 1000-row list keeps pace through flings', { id: 'PERF-LIST-001', covers: ['view:list'] }, async (t) => {
  await scene(t, 'list', 1000);
  const list = t.app.view.testId('perf-list');
  const started = Date.now();
  for (const direction of ['up', 'up', 'down', 'up'] as const) {
    await t.app.view.fling(direction, 4000, { at: list });
    await glideSettled(t);
  }
  await new Promise<void>(resolve => setTimeout(resolve, 1000));
  await expectFrames(t, await t.app.metrics({ windowMs: Date.now() - started }));
});

spec('a 1000-row list keeps pace while Logic moves its anchor every tick', { id: 'PERF-DATA-001', covers: ['view:list'] }, async (t) => {
  await scene(t, 'data', 1000);
  await expectFrames(t, await runBudget(t, 'data', 1000));
});

spec('250 springs at once keep pace', { id: 'PERF-ANIM-001' }, async (t) => {
  await scene(t, 'animation', 250);
  await expectFrames(t, await runBudget(t, 'animation', 250));
});

spec('eight translucent, blurred and shadowed layers keep pace', { id: 'PERF-FILL-001' }, async (t) => {
  await scene(t, 'fill', 8);
  await expectFrames(t, await runBudget(t, 'fill', 8));
});

spec('a mixed-script paragraph relayouts while it grows', { id: 'PERF-TEXT-001' }, async (t) => {
  await scene(t, 'text', 4);
  await expectFrames(t, await runBudget(t, 'text', 4));
});

spec('images decoding while a list scrolls keep pace', { id: 'PERF-IMAGE-001' }, async (t) => {
  await scene(t, 'images', 96);
  await expectFrames(t, await runBudget(t, 'images', 96));
});

spec('a dragged box shows the finger within the input budget', { id: 'PERF-TOUCH-001' }, async (t) => {
  await scene(t, 'touch', 48);
  const box = t.app.view.testId('perf-box');
  const started = Date.now();
  await t.app.view.drag(box, { dx: 220, dy: 0, durationMs: 900 });
  await t.app.view.drag(box, { dx: -220, dy: 0, durationMs: 900 });
  const metrics = await t.app.metrics({ windowMs: Date.now() - started });
  await t.attach('metrics', metrics);
  const verdict = judgeInput(metrics);
  await t.attach('verdict', verdict.line);
  expect(metrics.inputToPresent.count).toBeGreaterThan(5);
  expect(verdict.pass, verdict.line).toBe(true);
});

spec('pushed and popped pages show their first frame within budget', { id: 'PERF-NAV-001', covers: ['lx.navigateTo', 'lx.navigateBack'] }, async (t) => {
  await scene(t, 'transition', 100);
  const back = await runBudget(t, 'transition', 100);
  await t.attach('pop-metrics', back);
  const popped = judgeReveal(back);
  await t.attach('pop-verdict', popped.line);
  expect(back.reveals).toBeGreaterThan(2);
  expect(popped.pass, popped.line).toBe(true);
  const pushes: number[] = [];
  for (let round = 0; round < 3; round++) {
    await t.app.nav.to({ page: 'perf-target', query: { rows: '100' } });
    await t.app.view.testId('perf-target').waitFor({ state: 'visible' });
    // Each push reloads the page's module; its reveal statistics outlive the reload and would
    // report an earlier push, so read the time from this load to its first present.
    const pushed = await t.waitFor(() => t.app.metrics(), { until: m => m.firstPresentAt !== null });
    pushes.push(pushed.firstPresentAt!);
    await t.app.nav.back();
  }
  await t.attach('push-first-present-ms', pushes);
  expect(Math.max(...pushes)).toBeLessThanOrEqual(REVEAL_P95_MS);
});

spec('the list at its largest row count stays windowed and the page responsive', { id: 'PERF-LIST-MAX-001', covers: ['view:list'] }, async (t) => {
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

spec('a page past its View budget shows recovery chrome while the rest of the app works', { id: 'PERF-FAIL-001', requires: { chrome: true } }, async (t) => {
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
  expect(shown).toBeGreaterThan(tick * TICK_SHARE);
});
