import type { DriveRequest, PerfData } from '../../shared/perf-types';

const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
const TICK_MS = 16;

// Drivers for the workloads; the View measures the frames they cause. `drive` returns at
// once so the caller can measure while the scene runs.
Page({
  data: { running: '', tick: 0, phase: false } as PerfData,
  drive(request: DriveRequest) {
    void this.run(request);
  },
  async run({ scene, ms, count }: DriveRequest) {
    const until = Date.now() + ms;
    this.setData({ running: scene, tick: 0, phase: false });
    try {
      if (scene === 'transition') {
        while (Date.now() < until) {
          await lx.navigateTo({ page: 'perf-target', query: { rows: String(count) } });
          await sleep(400);
          await lx.navigateBack();
          await sleep(400);
        }
      } else if (scene === 'animation') {
        for (let phase = true; Date.now() < until; phase = !phase) {
          this.setData({ phase });
          await sleep(600);
        }
      } else if (scene === 'list' || scene === 'images' || scene === 'text') {
        for (let tick = 1; Date.now() < until; tick++) {
          this.setData({ tick });
          await sleep(TICK_MS);
        }
      } else {
        // Fill animates on its own and touch waits for a finger.
        await sleep(ms);
      }
    } finally {
      this.setData({ running: '', phase: false });
    }
  },
});
