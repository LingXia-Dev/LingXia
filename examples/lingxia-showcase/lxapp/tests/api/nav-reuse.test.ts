import type { PageInfo } from '@lingxia/types/automation';
import { expect, test, type TestApp } from '@lingxia/test';
import { bindFixture, eventually } from '@lingxia/test/showcase/helpers/poll.js';
import { SHOWCASE_APP_ID } from '@lingxia/test/showcase/helpers/app.js';

interface NavLab {
  firstVisible: boolean | null;
  visibleLog: boolean[];
  shows: (Record<string, unknown> | null)[];
  current: Record<string, unknown>[];
}

async function waitForCurrent(app: TestApp, name: string): Promise<PageInfo> {
  return eventually(() => app.nav.current(), (current) => current.name === name && current.ready, {
    describe: `current page '${name}' to become ready`,
    timeoutMs: 30_000,
  });
}

async function stackNames(app: TestApp): Promise<string[]> {
  return (await app.nav.stack()).map((page) => page.name ?? '');
}

async function navLab(app: TestApp): Promise<NavLab | null> {
  return app.logic.eval(({ pageStack }) => {
    const page = pageStack().find((candidate) => candidate.route.includes('/nav-lab/'));
    return page ? (JSON.parse(JSON.stringify(page.data)) as NavLab) : null;
  });
}

async function navCurrent(app: TestApp): Promise<unknown> {
  return JSON.parse(await app.logic.eval(({ lx }) => JSON.stringify(lx.nav.current.get())));
}

async function startAtHome(app: TestApp): Promise<void> {
  await app.nav.reset({ page: 'home' });
  await waitForCurrent(app, 'home');
}

test('reuse returns to the open instance and reports one landing', {
  id: 'NAV-REUSE-001',
  covers: ['lx.nav.go', 'lx.nav.current'],
  app: SHOWCASE_APP_ID,
  timeout: 120_000,
}, async (t) => {
  const { app } = bindFixture(t, 'NAV-REUSE-001');
  await startAtHome(app);

  await t.step('a page opened with params is visible as setup runs and gets them on its first onShow', async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.nav.go({ page: 'nav-lab', params: { step: '1' } }).opened;
    });
    await waitForCurrent(app, 'nav-lab');
    const lab = await eventually(() => navLab(app), (lab) => (lab?.shows.length ?? 0) === 1, {
      describe: 'nav-lab to record its first onShow',
    });
    expect(lab?.firstVisible).toBe(true);
    expect(lab?.visibleLog).toEqual([true]);
    expect(lab?.shows).toEqual([{ step: '1' }]);
    expect(await navCurrent(app)).toEqual({ page: 'nav-lab', params: { step: '1' } });
  });

  await t.step('reuse closes the pages above, hands its params to one onShow, and lands once', async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.nav.go({ page: 'device' }).opened;
      await lx.nav.go({ page: 'media' }).opened;
    });
    await waitForCurrent(app, 'media');
    expect(await stackNames(app)).toEqual(['home', 'nav-lab', 'device', 'media']);

    const landings = await app.logic.eval(async ({ lx }) => {
      const seen: (string | null)[] = [];
      const stop = lx.nav.current.watch((current) => seen.push(current?.page ?? null));
      await lx.nav.go({ page: 'nav-lab', params: { step: '2' } }, { reuse: true });
      await new Promise<void>((resolve) => setTimeout(() => resolve(), 500));
      stop();
      return seen;
    });
    expect(landings).toEqual(['media', 'nav-lab']);
    expect(await stackNames(app)).toEqual(['home', 'nav-lab']);
    const lab = await eventually(() => navLab(app), (lab) => (lab?.shows.length ?? 0) === 2, {
      describe: 'nav-lab to record the onShow that revealed it',
    });
    expect(lab?.shows).toEqual([{ step: '1' }, { step: '2' }]);
    expect(lab?.visibleLog).toEqual([true, false, true]);
    // The reuse's params are the instance's params now, for every reader.
    expect(lab?.current).toEqual([{ step: '1' }, { step: '2' }]);
    expect(await navCurrent(app)).toEqual({ page: 'nav-lab', params: { step: '2' } });
  });

  await t.step('pages a reuse closes settle canceled before the survivor is shown', async () => {
    const order = await app.logic.eval(async ({ lx }) => {
      const log: string[] = [];
      (globalThis as { navOrder?: string[] }).navOrder = log;
      const device = lx.nav.go({ page: 'device' });
      await device.opened;
      void device.result.then((result) => {
        const canceled = result as { status: string; reason?: string };
        log.push(`result:${canceled.status}:${canceled.reason}`);
      });
      await lx.nav.go({ page: 'nav-lab', params: { step: '3' } }, { reuse: true });
      await new Promise<void>((resolve) => setTimeout(() => resolve(), 300));
      delete (globalThis as { navOrder?: string[] }).navOrder;
      return log;
    });
    expect(order).toEqual(['result:canceled:user', 'show:{"step":"3"}']);
  });

  await t.step('reusing the top page shows it again only with params', async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.nav.go({ page: 'nav-lab', params: { step: '4' } }, { reuse: true });
    });
    let lab = await eventually(() => navLab(app), (lab) => (lab?.shows.length ?? 0) === 4, {
      describe: 'nav-lab to be shown again with the reuse params',
    });
    expect(lab?.shows[3]).toEqual({ step: '4' });
    expect(await navCurrent(app)).toEqual({ page: 'nav-lab', params: { step: '4' } });

    await app.logic.eval(async ({ lx }) => {
      await lx.nav.go({ page: 'nav-lab' }, { reuse: true });
      await new Promise<void>((resolve) => setTimeout(() => resolve(), 300));
    });
    lab = await navLab(app);
    expect(lab?.shows.length).toBe(4);
    expect(await navCurrent(app)).toEqual({ page: 'nav-lab', params: { step: '4' } });
  });

  await t.step('moves run in order, and an identical waiting one joins it', async () => {
    const joined = await app.logic.eval(async ({ lx }) => {
      const first = lx.nav.go({ page: 'device' });
      const again = lx.nav.go({ page: 'device' });
      const back = lx.nav.back();
      const media = lx.nav.go({ page: 'media' });
      await Promise.all([first.opened, back, media.opened]);
      return first === again;
    });
    expect(joined).toBe(true);
    await waitForCurrent(app, 'media');
    expect(await stackNames(app)).toEqual(['home', 'nav-lab', 'media']);
  });

  await t.step('closing a page an earlier move closed does nothing', async () => {
    const outcome = await app.logic.eval(async ({ lx }) => {
      const device = lx.nav.go({ page: 'device' });
      await device.opened;
      const back = lx.nav.back();
      const closed = device.close().then(() => 'resolved', (error: { code?: string }) => error.code ?? 'unknown');
      await back;
      const result = (await device.result) as { status: string };
      return { closed: await closed, result: result.status };
    });
    expect(outcome).toEqual({ closed: 'resolved', result: 'canceled' });
    expect(await stackNames(app)).toEqual(['home', 'nav-lab', 'media']);
  });

  await t.step('a storm of moves is bounded', async () => {
    const counts = await app.logic.eval(async ({ lx }) => {
      const waiting: Promise<unknown>[] = [];
      let refused = 0;
      for (let i = 0; i < 20; i += 1) {
        try {
          waiting.push(i % 2 === 0 ? lx.nav.go({ page: 'device' }).opened : lx.nav.back());
        } catch (error) {
          if ((error as { code?: string }).code === 'E_INVALID_STATE') refused += 1;
          else throw error;
        }
      }
      await Promise.all(waiting);
      return { admitted: waiting.length, refused };
    });
    expect(counts).toEqual({ admitted: 8, refused: 12 });
    await waitForCurrent(app, 'media');
    expect(await stackNames(app)).toEqual(['home', 'nav-lab', 'media']);
  });

  await t.step('with one open, replace is ignored; with none open, reuse is a plain go', async () => {
    await app.logic.eval(async ({ lx }) => {
      await lx.nav.go({ page: 'device' }).opened;
      await lx.nav.go({ page: 'nav-lab' }, { reuse: true, replace: true });
    });
    await waitForCurrent(app, 'nav-lab');
    expect(await stackNames(app)).toEqual(['home', 'nav-lab']);

    await app.logic.eval(async ({ lx }) => {
      await lx.nav.go({ page: 'wifi' }, { reuse: true });
    });
    await waitForCurrent(app, 'wifi');
    expect(await stackNames(app)).toEqual(['home', 'nav-lab', 'wifi']);

    await app.logic.eval(async ({ lx }) => {
      await lx.nav.go({ page: 'device' }, { reuse: true, replace: true });
    });
    await waitForCurrent(app, 'device');
    expect(await stackNames(app)).toEqual(['home', 'nav-lab', 'device']);
  });

  await t.step('reuse takes only a page target', async () => {
    const code = await app.logic.eval(({ lx }) => {
      try {
        // @ts-expect-error a tab is switched to, not reused
        void lx.nav.go({ tab: 'todo' }, { reuse: true });
        return 'accepted';
      } catch (error) {
        return (error as { code?: string }).code ?? 'unknown';
      }
    });
    expect(code).toBe('E_INVALID_ARG');
  });

  await startAtHome(app);
});

test('a tab keeps its params until a switch carries new ones', {
  id: 'NAV-REUSE-002',
  covers: ['lx.nav.go', 'lx.nav.current'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'NAV-REUSE-002');
  await startAtHome(app);
  await app.logic.eval(async ({ lx }) => {
    await lx.nav.go({ tab: 'todo', params: { filter: 'open' } });
  });
  await waitForCurrent(app, 'todo');
  expect(await navCurrent(app)).toEqual({ page: 'todo', params: { filter: 'open' } });

  await app.logic.eval(async ({ lx }) => {
    await lx.nav.go({ tab: 'home' });
    await lx.nav.go({ tab: 'todo' });
  });
  await waitForCurrent(app, 'todo');
  expect(await navCurrent(app)).toEqual({ page: 'todo', params: { filter: 'open' } });

  await app.logic.eval(async ({ lx }) => {
    await lx.nav.go({ tab: 'todo', params: { filter: 'done' } });
  });
  expect(await navCurrent(app)).toEqual({ page: 'todo', params: { filter: 'done' } });
  await startAtHome(app);
});
