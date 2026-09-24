import { expect, spec } from '@lingxia/test';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';

const STATUS = 'https://api.example.com/lingxia-showcase/clock/status';
const START = Date.UTC(2030, 0, 1);

spec("advance a Logic polling loop with the test clock", {
  id: "AUT-CLOCK-001",
  covers: ['ClockDriver.install', 'ClockDriver.tick', 'ClockDriver.setSystemTime', 'ClockDriver.uninstall'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-CLOCK-001");
  await app.network.route(STATUS, { json: { online: true } });

  expect(await app.clock.install({ now: START })).toBe(START);

  // A poll every 3 s that awaits a routed fetch and its body before it
  // re-arms, as an app's status page would.
  await app.eval({
    script: `
      const state = globalThis.__clockPolls = { at: [], online: [], timer: null };
      const poll = async () => {
        const response = await fetch(${JSON.stringify(STATUS)});
        const body = await response.json();
        state.at.push(Date.now());
        state.online.push(body.online);
        state.timer = setTimeout(poll, 3000);
      };
      state.timer = setTimeout(poll, 3000);
      return new Date().toISOString();
    `,
  });
  const polls = () => app.eval<{ at: number[]; online: boolean[] }>({
    script: '({ at: globalThis.__clockPolls.at, online: globalThis.__clockPolls.online })',
  });

  // Nothing fires on real time while the clock is installed, and the runner's
  // own timers are not faked: this wait is real.
  await new Promise<void>((resolve) => setTimeout(resolve, 100));
  expect((await polls()).at).toEqual([]);

  const first = await app.clock.tick(9_000);
  expect(first.fired).toBe(3);
  expect(first.pending).toBe(1);
  expect(first.now).toBe(START + 9_000);
  expect(await polls()).toEqual({
    at: [START + 3_000, START + 6_000, START + 9_000],
    online: [true, true, true],
  });

  // A time jump moves Date, not the timers.
  expect(await app.clock.setSystemTime(START + 60_000)).toBe(START + 60_000);
  const jumped = await app.clock.tick(2_999);
  expect(jumped.fired).toBe(0);
  const next = await app.clock.tick(1);
  expect(next.fired).toBe(1);
  const after = (await polls()).at;
  expect(after[after.length - 1]).toBe(START + 60_000 + 3_000);

  const removed = await app.clock.uninstall();
  expect(removed).toEqual({ uninstalled: true, dropped: 1 });
  const realNow = await app.eval<number>({ script: 'Date.now()' });
  expect(Math.abs(realNow - Date.now()) < 60_000).toBeTruthy();
});

spec("refuse a second install and calls without a clock", {
  id: "AUT-CLOCK-002",
  covers: ['ClockDriver.install', 'ClockDriver.tick', 'ClockDriver.runAll', 'ClockDriver.uninstall'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-CLOCK-002");

  await t.reject(() => app.clock.tick(10), { code: 'E_CLOCK_NOT_INSTALLED' });
  await app.clock.install();
  await t.reject(() => app.clock.install(), { code: 'E_CLOCK_INSTALLED' });

  await app.eval({
    script: `
      globalThis.__clockLog = [];
      setTimeout(() => {
        globalThis.__clockLog.push('outer');
        setTimeout(() => globalThis.__clockLog.push('inner'), 500);
      }, 1000);
      return true;
    `,
  });
  const drained = await app.clock.runAll();
  expect(drained.fired).toBe(2);
  expect(drained.pending).toBe(0);
  expect(await app.eval({ script: 'globalThis.__clockLog' })).toEqual(['outer', 'inner']);

  await app.eval({ script: 'globalThis.__clockBeat = setInterval(() => {}, 10); return true;' });
  await t.reject(() => app.clock.runAll({ maxTimers: 20 }), { message: /still pending/ });
  // Left installed on purpose: the spec's end uninstalls it.
});

spec("reject the test clock from inside app Logic", {
  id: "AUT-CLOCK-003",
  covers: ['ClockDriver.install'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-CLOCK-003");

  const rejection = await app.eval({
    script: `
      const clock = lx.automation().lxapp().clock;
      try {
        await clock.install();
        return { readable: typeof clock.install === 'function', rejected: false };
      } catch (error) {
        return {
          readable: typeof clock.install === 'function',
          rejected: true,
          code: String(error?.code || ''),
          message: String(error?.message || error),
        };
      }
    `,
  }) as { readable: boolean; rejected: boolean; code?: string; message?: string };

  expect(rejection.readable).toBe(true);
  expect(rejection.rejected).toBeTruthy();
  expect(rejection.code).toBe('E_AUTOMATION');
  expect(rejection.message).toContain('host automation run');
});
