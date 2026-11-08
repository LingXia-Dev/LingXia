import type { BenchRequest, BenchResult, DriveRequest, PerfData, PerfRow } from '../../shared/perf-types';

const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

// Drivers for the workloads; the View measures the frames they cause. `drive` returns at
// once so the caller can measure while the scene runs.
Page({
  data: { running: '', tick: 0, phase: false, rows: [] } as PerfData,
  drive(request: DriveRequest) {
    void this.run(request);
  },
  ping() {
    return this.data.tick;
  },
  // Latencies are read from the runtime's bridge probes (`lxdev logs --grep bridge-probe`).
  async bench({ writes, requests, calls }: BenchRequest): Promise<BenchResult> {
    for (let tick = 1; tick <= writes; tick++) {
      this.setData({ tick });
      await this.flush();
    }
    // Only desktop answers `ui.*` in the View; mobile shows toasts natively.
    const os = lx.getDeviceInfo().osName;
    const viewUi = os === 'macOS' || os === 'Windows';
    if (viewUi) for (let i = 0; i < requests; i++) await lx.hideToast().catch(() => undefined);
    const record = Object.fromEntries(Array.from({ length: 20 }, (_, i) => [`field${i}`, `value ${i}`]));
    const started = Date.now();
    for (let i = 0; i < calls; i++) this.setData({ record: { ...record, field0: i } });
    const setDataUs = ((Date.now() - started) * 1000) / Math.max(1, calls);
    await this.flush();
    this.setData({ record: null });
    return viewUi ? { setDataUs } : { setDataUs, requestsSkipped: `${os} answers ui.* natively, not in the View` };
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
      } else if (scene === 'state') {
        // A Logic-owned list of records: one record changes per tick, the rest stays.
        const rows: PerfRow[] = Array.from({ length: count }, (_, id) => ({
          id, title: `Record ${id + 1}`, note: 'A second line of text kept in Logic data', count: 0, done: false,
        }));
        this.setData({ rows });
        await this.flush();
        for (let tick = 1; Date.now() < until; tick++) {
          const row = (tick * 7) % count;
          this.setData({ tick });
          this.setPath(['rows', row, 'count'], tick);
          this.setPath(['rows', row, 'done'], tick % 2 === 0);
          await this.flush();
        }
      } else if (scene === 'data' || scene === 'images' || scene === 'text') {
        // A GPU page acknowledges a write once the frame showing it is on screen, so the
        // loop writes once per presented frame.
        for (let tick = 1; Date.now() < until; tick++) {
          this.setData({ tick });
          await this.flush();
        }
      } else {
        // List glides natively, fill animates on its own and touch waits for a finger.
        await sleep(ms);
      }
    } finally {
      this.setData({ running: '', phase: false, rows: [] });
    }
  },
});
