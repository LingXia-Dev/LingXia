import type { BenchRequest, BenchResult, DriveRequest, PerfData, PerfRow } from '../../shared/perf-types';
import { definePage } from '@lingxia/types/page';

const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

// Drivers for the workloads; the View measures the frames they cause. `drive` returns at
// once so the caller can measure while the scene runs.
export default definePage({
  data: { running: '', tick: 0, phase: false, rows: [] } as PerfData,
  setup(page) {
    const actions = {
      drive(request: DriveRequest) {
        void actions.run(request);
      },
      ping() {
        return page.data.tick;
      },
      // Latencies are read from the runtime's bridge probes (`lxdev logs --grep bridge-probe`).
      async bench({ writes, requests, calls }: BenchRequest): Promise<BenchResult> {
        for (let tick = 1; tick <= writes; tick++) {
          page.update(d => { d.tick = tick; });
          await page.flush();
        }
        // Only desktop answers `ui.*` in the View; mobile shows toasts natively.
        const os = lx.device.info.osName;
        const viewUi = os === 'macOS' || os === 'Windows';
        if (viewUi) {
          for (let i = 0; i < requests; i++) {
            const toast = await lx.ui.toast({ title: 'bench', durationMs: 1 }).catch(() => undefined);
            await toast?.dismiss().catch(() => undefined);
          }
        }
        const record = Object.fromEntries(Array.from({ length: 20 }, (_, i) => [`field${i}`, `value ${i}`]));
        const started = Date.now();
        for (let i = 0; i < calls; i++) page.update(d => { d.record = { ...record, field0: i }; });
        const setDataUs = ((Date.now() - started) * 1000) / Math.max(1, calls);
        await page.flush();
        page.update(d => { d.record = null; });
        return viewUi ? { setDataUs } : { setDataUs, requestsSkipped: `${os} answers ui.* natively, not in the View` };
      },
      async run({ scene, ms, count }: DriveRequest) {
        const until = Date.now() + ms;
        page.update(d => {
          d.running = scene;
          d.tick = 0;
          d.phase = false;
        });
        try {
          if (scene === 'transition') {
            while (Date.now() < until) {
              await lx.nav.go({ page: 'perf-target', params: { rows: String(count) } }).opened;
              await sleep(400);
              await lx.nav.back();
              await sleep(400);
            }
          } else if (scene === 'animation') {
            for (let phase = true; Date.now() < until; phase = !phase) {
              page.update(d => { d.phase = phase; });
              await sleep(600);
            }
          } else if (scene === 'state') {
            // A Logic-owned list of records: one record changes per tick, the rest stays.
            const rows: PerfRow[] = Array.from({ length: count }, (_, id) => ({
              id, title: `Record ${id + 1}`, note: 'A second line of text kept in Logic data', count: 0, done: false,
            }));
            page.update(d => { d.rows = rows; });
            await page.flush();
            for (let tick = 1; Date.now() < until; tick++) {
              const row = (tick * 7) % count;
              page.update(d => { d.tick = tick; });
              page.update(d => { d['rows'][row]['count'] = tick; });
              page.update(d => { d['rows'][row]['done'] = tick % 2 === 0; });
              await page.flush();
            }
          } else if (scene === 'data' || scene === 'images' || scene === 'text') {
            // A GPU page acknowledges a write once the frame showing it is on screen, so the
            // loop writes once per presented frame.
            for (let tick = 1; Date.now() < until; tick++) {
              page.update(d => { d.tick = tick; });
              await page.flush();
            }
          } else {
            // List glides natively, fill animates on its own and touch waits for a finger.
            await sleep(ms);
          }
        } finally {
          page.update(d => {
            d.running = '';
            d.phase = false;
            d.rows = [];
          });
        }
      },
    };
    return {
      actions,
    };
  },
});
