import { expect, test } from '@lingxia/test';
import { rawAutomation } from '@lingxia/test/runner';

/** The first feedback of a session is visible within this of the Logic call. */
const FIRST_BUDGET_MS = 200;
/** Every later one, after the layer closed too, is one frame of View work. */
const FRAME_BUDGET_MS = 16.7;
const ROUNDS = 10;

const layer = () => rawAutomation().lxapp().feedback;

async function layerClosed(): Promise<boolean> {
  try {
    await layer().uiFrame();
    return false;
  } catch (error) {
    if ((error as { code?: string }).code === 'E_PAGE_NOT_READY') return true;
    throw error;
  }
}

// Each round waits for the layer to close, so each toast opens it again: the first may be the
// session's first, every later one finds the surface the first one left.
test('a toast after the layer closed shows on the surface it kept, in one frame of work', {
  id: 'PERF-TOAST-001', covers: ['lx.ui.toast'],
  start: { page: 'home' },
}, async (t) => {
  const app = t.app;
  const latency: number[] = [];
  const work: number[] = [];
  const surfaces = new Set<number>();
  for (let round = 0; round < ROUNDS; round++) {
    await t.waitFor(layerClosed, { until: closed => closed, timeout: 15_000 });
    const calledAt = await app.logic.eval(({ lx }, round) => {
      const at = Date.now();
      lx.ui.toast(`Toast ${round}`, { durationMs: 600 });
      return at;
    }, round);
    // Read as soon as it shows: the last present then is the toast's first frame or soon after.
    const shown = await t.waitFor(() => layer().uiFrame().catch(() => null), {
      until: frame => (frame?.lastPresentEpochMs ?? 0) >= calledAt, timeout: 10_000,
    });
    await expect(app.feedback.view.testId('lx-toast')).toContainText(`Toast ${round}`);
    latency.push(shown!.lastPresentEpochMs! - calledAt);
    work.push(shown!.evalMs.last);
    // One page for the run: its first present stays the same while the surface is kept.
    surfaces.add(shown!.firstPresentAt!);
  }
  const sorted = [...latency].sort((a, b) => a - b);
  const summary = `first ${latency[0]} ms, later p50 ${sorted[Math.floor(ROUNDS / 2)]}, max ${sorted[ROUNDS - 1]} ms · View work max ${Math.max(...work)} ms · ${surfaces.size} surface(s)`;
  await t.attach('toast-latency', { latencyMs: latency, workMs: work, surfaces: surfaces.size });
  expect(latency[0], summary).toBeLessThanOrEqual(FIRST_BUDGET_MS);
  expect(surfaces.size, summary).toBe(1);
  expect(Math.max(...work.slice(1)), summary).toBeLessThanOrEqual(FRAME_BUDGET_MS);
});
