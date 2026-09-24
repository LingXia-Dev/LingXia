import type {
  ClockDriver,
  ClockInstallOptions,
  ClockRunAllOptions,
  ClockTime,
  ClockUninstallResult,
  HostRunAutomation,
} from "@lingxia/types/automation";

/** The fixture surface the clock wrapper needs. */
export interface ClockHost {
  act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T>;
  defer(cleanup: () => void | Promise<void>): void;
}

/**
 * Clocks one spec installed, by lxapp id. They are uninstalled when the spec
 * ends, so the next spec starts on real time; the host drops whatever is left
 * at run end, and an app that reopened is back on real time already.
 */
export class ClockScope {
  readonly apps = new Set<string>();
  /** Test timers still pending when the spec's clocks were uninstalled. */
  dropped = 0;
  private cleanupRegistered = false;

  track(host: ClockHost, appid: string, automation: () => HostRunAutomation): void {
    this.apps.add(appid);
    if (this.cleanupRegistered) return;
    this.cleanupRegistered = true;
    host.defer(async () => {
      for (const id of this.apps) {
        // Select the app again: a profile rollback may have reopened it.
        // Raw driver: cleanup runs after the fixture closed its action guard.
        try {
          const result: ClockUninstallResult = await automation().lxapp(id).clock.uninstall();
          this.dropped += result.dropped;
        } catch { /* the run end restores real time */ }
      }
      this.apps.clear();
    });
  }
}

/** `install({ now })` / `setSystemTime` detail for the trace. */
function describeTime(time: ClockTime | undefined): string {
  if (time === undefined) return "";
  if (time instanceof Date) return Number.isNaN(time.getTime()) ? "Invalid Date" : time.toISOString();
  return String(time);
}

/** A `Date` crosses to the host as epoch milliseconds. */
function toWire(time: ClockTime): number | string {
  return time instanceof Date ? time.getTime() : time;
}

/**
 * `resolve` reads the raw driver lazily, inside each traced call: a host
 * without the test clock then fails that call, never the `t.app.clock` read.
 */
export function wrapClock(
  resolve: () => { driver: ClockDriver | undefined; appid: () => Promise<string> },
  host: ClockHost,
  scope: ClockScope,
  automation: () => HostRunAutomation,
): ClockDriver {
  const driver = (): ClockDriver => {
    const clock = resolve().driver;
    if (!clock) throw new Error("t.app.clock is not supported by this host; update the LingXia host");
    return clock;
  };
  return {
    install: (options?: ClockInstallOptions) =>
      host.act("clock.install", describeTime(options?.now), async () => {
        const clock = driver();
        const now = options?.now === undefined ? undefined : toWire(options.now);
        // An explicit `undefined` argument is not "no options" to the host:
        // omit it instead.
        const result = await (now === undefined ? clock.install() : clock.install({ now }));
        scope.track(host, await resolve().appid(), automation);
        return result;
      }),
    tick: (ms: number) => host.act("clock.tick", `${ms}ms`, () => driver().tick(ms)),
    runAll: (options?: ClockRunAllOptions) =>
      host.act("clock.runAll", options?.maxTimers === undefined ? "" : `max ${options.maxTimers}`, () =>
        options === undefined ? driver().runAll() : driver().runAll(options)),
    setSystemTime: (time: ClockTime) =>
      host.act("clock.setSystemTime", describeTime(time), () => driver().setSystemTime(toWire(time))),
    uninstall: () =>
      host.act("clock.uninstall", "", async () => {
        const result = await driver().uninstall();
        scope.dropped += result.dropped;
        return result;
      }),
  };
}
