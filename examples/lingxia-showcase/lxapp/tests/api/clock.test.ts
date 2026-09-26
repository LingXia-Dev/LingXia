import { expect, rawAutomation, spec } from '@lingxia/test';
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

  expect(await app.clock.install({ now: START })).toEqual({ now: START, pending: 0 });

  // A poll every 3 s that awaits a routed fetch and its body before it
  // re-arms, as an app's status page would.
  await app.logic.eval((_scope, url) => {
    const state = { at: [] as number[], online: [] as boolean[], timer: null as unknown };
    (globalThis as unknown as { __clockPolls: typeof state }).__clockPolls = state;
    const poll = async () => {
      const response = await fetch(url);
      const body = await response.json() as { online: boolean };
      state.at.push(Date.now());
      state.online.push(body.online);
      state.timer = setTimeout(poll, 3000);
    };
    state.timer = setTimeout(poll, 3000);
    return new Date().toISOString();
  }, STATUS);
  const polls = () => app.logic.eval(() => {
    const state = (globalThis as unknown as { __clockPolls: { at: number[]; online: boolean[] } }).__clockPolls;
    return { at: state.at, online: state.online };
  });

  // Nothing fires on real time while the clock is installed, and the runner's
  // own timers are not faked: this wait is real.
  await new Promise<void>((resolve) => setTimeout(resolve, 100));
  expect((await polls()).at).toEqual([]);

  expect(await app.clock.tick(9_000)).toEqual({ now: START + 9_000, fired: 3, pending: 1 });
  expect(await polls()).toEqual({
    at: [START + 3_000, START + 6_000, START + 9_000],
    online: [true, true, true],
  });

  // A time jump moves Date, not the timers.
  expect(await app.clock.setSystemTime(START + 60_000)).toEqual({ now: START + 60_000, pending: 1 });
  const jumped = await app.clock.tick(2_999);
  expect(jumped.fired).toBe(0);
  const next = await app.clock.tick(1);
  expect(next.fired).toBe(1);
  const after = (await polls()).at;
  expect(after[after.length - 1]).toBe(START + 60_000 + 3_000);

  // The pending poll is dropped, never fired; the report's diagnostics say so.
  expect(await app.clock.uninstall()).toBeUndefined();
  const realNow = await app.logic.eval(() => Date.now());
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

  await app.logic.eval(() => {
    const log: string[] = [];
    (globalThis as unknown as { __clockLog: string[] }).__clockLog = log;
    setTimeout(() => {
      log.push('outer');
      setTimeout(() => log.push('inner'), 500);
    }, 1000);
    return true;
  });
  const drained = await app.clock.runAll();
  expect(drained.fired).toBe(2);
  expect(drained.pending).toBe(0);
  expect(await app.logic.eval(() => (globalThis as unknown as { __clockLog: string[] }).__clockLog)).toEqual(['outer', 'inner']);

  await app.logic.eval(() => {
    (globalThis as unknown as { __clockBeat: unknown }).__clockBeat = setInterval(() => {}, 10);
    return true;
  });
  await t.reject(() => app.clock.runAll({ maxTimers: 20 }), { message: /still pending/ });
  // Left installed on purpose: the spec's end uninstalls it.
});

spec("reject the test clock from inside app Logic", {
  id: "AUT-CLOCK-003",
  covers: ['ClockDriver.install'],
  app: SHOWCASE_APP_ID,
}, async (t) => {
  const { app } = bindFixture(t, "AUT-CLOCK-003");

  // App Logic's driver type has no `clock`; the member is still readable.
  const rejection = await app.logic.eval(async ({ lx }) => {
    const clock = (rawAutomation().lxapp() as unknown as { clock: { install(): Promise<unknown> } }).clock;
    try {
      await clock.install();
      return { readable: typeof clock.install === 'function', rejected: false };
    } catch (error) {
      const failure = error as { code?: unknown; message?: unknown } | undefined;
      return {
        readable: typeof clock.install === 'function',
        rejected: true,
        code: String(failure?.code || ''),
        message: String(failure?.message || error),
      };
    }
  });

  expect(rejection.readable).toBe(true);
  expect(rejection.rejected).toBeTruthy();
  expect(rejection.code).toBe('E_AUTOMATION');
  expect(rejection.message).toContain('host automation run');
});
