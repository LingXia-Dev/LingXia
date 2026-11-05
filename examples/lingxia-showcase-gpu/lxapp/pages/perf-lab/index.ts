import type { DriveRequest, PerfData } from '../../shared/perf-types';

const TICK_MS = 16;

// Drivers for the workloads; the View measures the frames they cause. `drive` returns at
// once and each driver runs on timer callbacks: state set from an `await` continuation
// reaches the View far less often than once a frame.
Page({
  data: { running: '', tick: 0, phase: false } as PerfData,
  drive({ scene, ms, count }: DriveRequest) {
    const until = Date.now() + ms;
    const done = () => this.setData({ running: '', phase: false });
    this.setData({ running: scene, tick: 0, phase: false });
    if (scene === 'transition') {
      const cycle = () => {
        if (Date.now() >= until) return done();
        lx.navigateTo({ page: 'perf-target', query: { rows: String(count) } })
          .then(() => setTimeout(() => {
            lx.navigateBack().then(() => setTimeout(cycle, 400), done);
          }, 400), done);
      };
      cycle();
      return;
    }
    if (scene === 'fill' || scene === 'touch') {
      // Fill animates on its own and touch waits for a finger: nothing to change per tick.
      setTimeout(done, ms);
      return;
    }
    const every = scene === 'animation' ? 600 : TICK_MS;
    let tick = 0;
    const timer = setInterval(() => {
      if (Date.now() >= until) {
        clearInterval(timer);
        return done();
      }
      tick++;
      this.setData(scene === 'animation' ? { phase: tick % 2 === 1 } : { tick });
    }, every);
  },
});
