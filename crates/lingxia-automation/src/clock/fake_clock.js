// Test clock for one lxapp Logic context. Evaluated once per Logic context of
// an automation-enabled host; it stays dormant (no global changes) until the
// host driver calls `install`. The controller lives under a registered symbol
// and every method takes the install token, so a stale caller cannot drive a
// clock another install owns.
//
// While installed, `Date` (no-argument construction, `Date()`, `Date.now`),
// `setTimeout`/`clearTimeout`, `setInterval`/`clearInterval` and
// `performance.now` read the fake clock. Timers only fire from `tick` /
// `runAll`, in time order (ties in scheduling order). After each firing the
// context is given real event-loop turns until it goes quiet, so promise
// chains a callback started (an `await fetch(...)` against a fulfilled route,
// `await storage.get(...)`) progress before the next timer fires.
//
// Settling awaits `hostTurn`, a host round trip rather than a real
// zero-delay timer, which the runtime can drop; a lost turn would stall the
// tick forever. The lease watchdog and error rethrows use the natives
// captured here, before any install, and never the fake ones.
(function (leased, hostTurn, realSetTimeout, realSetInterval, realClearInterval) {
  'use strict';
  const KEY = Symbol.for('lingxia.automation.clock');
  if (Object.prototype.hasOwnProperty.call(globalThis, KEY)) return;

  // Fake ids live far above the real registry's counter, which starts at 0,
  // so a stale fake id handed to the real clearTimeout after uninstall
  // (ToUint32) cannot cancel a real timer.
  const FAKE_ID_BASE = 0xc0000000;
  const SETTLE_MIN_TURNS = 3;
  const SETTLE_MAX_TURNS = 100;
  const TICK_MAX_FIRINGS = 10000;
  const WATCH_MS = 1000;

  let clock = null;
  // Fake timer ids keep counting across installs, so an id a stale caller
  // still holds never names a timer of a later clock.
  let nextId = FAKE_ID_BASE;

  // Real turns until one passes with no timer scheduled or cleared, after at
  // least SETTLE_MIN_TURNS: enough for promise chains and native work that
  // completes at once; bounded so a busy loop cannot stall the tick.
  const settle = async function (c) {
    for (let i = 0; i < SETTLE_MAX_TURNS; i++) {
      const before = c.activity;
      await hostTurn();
      if (clock !== c) return;
      if (i + 1 >= SETTLE_MIN_TURNS && c.activity === before) return;
    }
  };

  const earliest = function (c) {
    let next = null;
    for (const timer of c.timers.values()) {
      if (next === null || timer.at < next.at || (timer.at === next.at && timer.seq < next.seq)) {
        next = timer;
      }
    }
    return next;
  };

  const advanceTo = function (c, mono) {
    if (mono <= c.mono) return;
    const delta = mono - c.mono;
    c.mono = mono;
    c.wall += delta;
  };

  const fire = function (c, timer) {
    if (timer.interval > 0) {
      timer.at += timer.interval;
      timer.seq = ++c.seq;
    } else {
      c.timers.delete(timer.id);
    }
    try {
      timer.fn.apply(undefined, timer.args);
    } catch (error) {
      // The real registry cancels an interval whose callback throws and
      // reports nothing itself; rethrowing on a real turn surfaces the error
      // wherever an uncaught timer error would go.
      if (timer.interval > 0) c.timers.delete(timer.id);
      realSetTimeout(function () { throw error; }, 0);
    }
  };

  const parseTime = function (value, RealDate) {
    if (value === undefined || value === null) return RealDate.now();
    const ms = typeof value === 'number' ? value : RealDate.parse(String(value));
    return Number.isFinite(ms) ? ms : NaN;
  };

  const install = function (token, now) {
    if (clock !== null) {
      if (clock.token === token || leased(clock.token)) return { error: 'installed' };
      // Left behind by a run that has ended; its watchdog has not run yet.
      uninstallClock(clock);
    }
    const RealDate = globalThis.Date;
    const wall = parseTime(now, RealDate);
    if (Number.isNaN(wall)) return { error: 'invalid_time' };
    const perf = globalThis.performance;
    const perfNow = perf && typeof perf.now === 'function' ? perf.now : null;
    const c = {
      token,
      mono: 0,
      wall,
      perfBase: perfNow ? perfNow.call(perf) : 0,
      timers: new Map(),
      seq: 0,
      activity: 0,
      busy: false,
      watch: null,
      saved: {
        setTimeout: globalThis.setTimeout,
        clearTimeout: globalThis.clearTimeout,
        setInterval: globalThis.setInterval,
        clearInterval: globalThis.clearInterval,
        Date: RealDate,
        perf: perfNow ? {
          target: perf,
          descriptor: Object.getOwnPropertyDescriptor(perf, 'now'),
        } : null,
      },
    };

    const schedule = function (fn, delay, args, repeat) {
      if (typeof fn !== 'function') throw new TypeError('timer callback must be a function');
      let ms = Number(delay);
      if (!Number.isFinite(ms) || ms < 0) ms = 0;
      ms = Math.floor(ms);
      // A zero-period interval would fire forever within one tick.
      if (repeat && ms < 1) ms = 1;
      const id = nextId++;
      c.timers.set(id, { id, at: c.mono + ms, seq: ++c.seq, interval: repeat ? ms : 0, fn, args });
      c.activity++;
      return id;
    };
    const cancel = function (id, real) {
      if (clock !== c) return real(id);
      if (typeof id === 'number' && c.timers.has(id)) {
        c.timers.delete(id);
        c.activity++;
        return undefined;
      }
      // A timer started before install is still a real one.
      if (!(typeof id === 'number' && id >= FAKE_ID_BASE)) return real(id);
      return undefined;
    };
    const nowMs = function () { return clock === c ? c.wall : c.saved.Date.now(); };

    // A stale wrapper (its clock is gone) hands off to the real timer; the
    // native may not pass extra arguments, so bind them here.
    const withArgs = function (fn, args) {
      if (args.length === 0 || typeof fn !== 'function') return fn;
      return function () { return fn.apply(this, args); };
    };
    function setTimeout(fn, delay, ...args) {
      if (clock !== c) return c.saved.setTimeout(withArgs(fn, args), delay);
      return schedule(fn, delay, args, false);
    }
    function setInterval(fn, delay, ...args) {
      if (clock !== c) return c.saved.setInterval(withArgs(fn, args), delay);
      return schedule(fn, delay, args, true);
    }
    function clearTimeout(id) { return cancel(id, c.saved.clearTimeout); }
    function clearInterval(id) { return cancel(id, c.saved.clearInterval); }

    function Date(...args) {
      if (!new.target) return new RealDate(nowMs()).toString();
      if (args.length === 0) return Reflect.construct(RealDate, [nowMs()], new.target);
      return Reflect.construct(RealDate, args, new.target);
    }
    // Shared prototype: `instanceof Date` holds for dates from either side.
    Date.prototype = RealDate.prototype;
    Date.now = nowMs;
    Date.parse = RealDate.parse;
    Date.UTC = RealDate.UTC;
    Object.defineProperty(Date, 'length', { value: 7 });

    globalThis.setTimeout = setTimeout;
    globalThis.clearTimeout = clearTimeout;
    globalThis.setInterval = setInterval;
    globalThis.clearInterval = clearInterval;
    globalThis.Date = Date;
    if (perfNow) {
      try {
        Object.defineProperty(perf, 'now', {
          configurable: true,
          writable: true,
          enumerable: false,
          value: function now() {
            return clock === c ? c.perfBase + c.mono : perfNow.call(perf);
          },
        });
      } catch (_) {
        // A sealed `performance` keeps real time.
        c.saved.perf = null;
      }
    }
    clock = c;
    c.watch = realSetInterval(function () {
      if (clock === c && !leased(c.token)) uninstallClock(c);
    }, WATCH_MS);
    return { ok: true, now: c.wall };
  };

  const uninstallClock = function (c) {
    const dropped = c.timers.size;
    c.timers.clear();
    if (c.watch !== null) realClearInterval(c.watch);
    c.watch = null;
    globalThis.setTimeout = c.saved.setTimeout;
    globalThis.clearTimeout = c.saved.clearTimeout;
    globalThis.setInterval = c.saved.setInterval;
    globalThis.clearInterval = c.saved.clearInterval;
    globalThis.Date = c.saved.Date;
    const perf = c.saved.perf;
    if (perf) {
      if (perf.descriptor) Object.defineProperty(perf.target, 'now', perf.descriptor);
      else delete perf.target.now;
    }
    if (clock === c) clock = null;
    return dropped;
  };

  const owned = function (token) {
    if (clock === null || clock.token !== token) return null;
    return clock;
  };

  const run = async function (token, limit, target) {
    const c = owned(token);
    if (c === null) return { error: 'not_installed' };
    if (c.busy) return { error: 'busy' };
    c.busy = true;
    let fired = 0;
    try {
      for (;;) {
        const next = earliest(c);
        if (next === null || (target !== null && next.at > target)) break;
        if (fired >= limit) return { error: 'limit', fired, pending: c.timers.size, now: c.wall };
        advanceTo(c, next.at);
        fire(c, next);
        fired++;
        await settle(c);
        if (clock !== c) return { error: 'not_installed' };
      }
      if (target !== null) {
        advanceTo(c, target);
        await settle(c);
        if (clock !== c) return { error: 'not_installed' };
      }
      return { ok: true, now: c.wall, fired, pending: c.timers.size };
    } finally {
      c.busy = false;
    }
  };

  const controller = Object.freeze({
    install,
    tick(token, ms) {
      const c = owned(token);
      return run(token, TICK_MAX_FIRINGS, c === null ? null : c.mono + ms);
    },
    runAll(token, maxTimers) {
      return run(token, maxTimers, null);
    },
    setSystemTime(token, time) {
      const c = owned(token);
      if (c === null) return { error: 'not_installed' };
      const wall = parseTime(time, c.saved.Date);
      if (Number.isNaN(wall)) return { error: 'invalid_time' };
      c.wall = wall;
      return { ok: true, now: c.wall, pending: c.timers.size };
    },
    state(token) {
      const c = owned(token);
      if (c === null) return { installed: clock !== null, owned: false };
      return { installed: true, owned: true, now: c.wall, pending: c.timers.size };
    },
    // Uninstall whatever clock is installed: the caller's run is ending or
    // replacing it, and nothing else may keep the app on fake time.
    uninstall() {
      if (clock === null) return { uninstalled: false, dropped: 0 };
      return { uninstalled: true, dropped: uninstallClock(clock) };
    },
  });
  Object.defineProperty(globalThis, KEY, { value: controller });
})
