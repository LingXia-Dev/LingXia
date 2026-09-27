import type {
  ClockAdvance,
  ClockDriver,
  ClockInstallOptions,
  ClockRunAllOptions,
  ClockState,
  ClockTime,
  ClockUninstallResult,
  HostRunAutomation,
} from "@lingxia/types/automation";
import type { TestClock } from "./types.js";

/** The fixture surface the clock wrapper needs. */
export interface ClockHost {
  act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T>;
  diagnostic(phase: string, message: string): void | Promise<void>;
}

/** The host's record, as a plain object of exactly these keys. */
function clockState({ now, pending }: ClockState): ClockState {
  return { now, pending };
}

function clockAdvance({ now, pending, fired }: ClockAdvance): ClockAdvance {
  return { now, pending, fired };
}

function droppedNote(dropped: number, when: string): string {
  return `${when} dropped ${dropped} pending test timer${dropped === 1 ? "" : "s"}; they never fired`;
}

/**
 * Clocks one spec installed, by lxapp id. The runner uninstalls them when
 * the spec ends, so the next spec starts on real time; an app that closed or
 * reopened since is back on real time already.
 */
export class ClockScope {
  readonly apps = new Set<string>();
  /** Test timers still pending when the spec's clocks were uninstalled. */
  dropped = 0;
  private automation: (() => HostRunAutomation) | undefined;

  track(appid: string, automation: () => HostRunAutomation): void {
    this.apps.add(appid);
    this.automation = automation;
  }

  /** Uninstall every tracked clock; rejects naming those that failed. */
  async reclaim(diagnostic: (phase: string, message: string) => void | Promise<void>): Promise<void> {
    const automation = this.automation;
    if (!automation) return;
    const failed: string[] = [];
    for (const id of [...this.apps]) {
      // Select the app again: a profile rollback may have reopened it.
      // Raw driver: the fixture is closed by now.
      try {
        const result: ClockUninstallResult = await automation().lxapp(id).clock.uninstall();
        this.dropped += result.dropped;
        if (result.dropped > 0) await diagnostic("clock", droppedNote(result.dropped, `the spec's end uninstalled the test clock of ${id} and`));
        this.apps.delete(id);
      } catch (error) {
        // A closed app took its Logic, and the clock with it.
        if (appGone(error)) { this.apps.delete(id); continue; }
        failed.push(`${id}: ${String((error as Error)?.message ?? error)}`);
      }
    }
    if (failed.length > 0) throw new Error(`test clocks were not uninstalled (${failed.join("; ")})`);
  }
}

/** The host's answer for an app that is not running. */
function appGone(error: unknown): boolean {
  return /lxapp is not active:/.test(String((error as Error)?.message ?? error));
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
): TestClock {
  const driver = (): ClockDriver => resolve().driver as ClockDriver;
  return {
    install: (options?: ClockInstallOptions) =>
      host.act("clock.install", describeTime(options?.now), async () => {
        const clock = driver();
        const now = options?.now === undefined ? undefined : toWire(options.now);
        // An explicit `undefined` argument is not "no options" to the host:
        // omit it instead.
        const result = await (now === undefined ? clock.install() : clock.install({ now }));
        scope.track(await resolve().appid(), automation);
        return clockState(result);
      }),
    tick: (ms: number) => host.act("clock.tick", `${ms}ms`, async () => clockAdvance(await driver().tick(ms))),
    runAll: (options?: ClockRunAllOptions) =>
      host.act("clock.runAll", options?.maxTimers === undefined ? "" : `max ${options.maxTimers}`, async () =>
        clockAdvance(await (options === undefined ? driver().runAll() : driver().runAll(options)))),
    setSystemTime: (time: ClockTime) =>
      host.act("clock.setSystemTime", describeTime(time), async () =>
        clockState(await driver().setSystemTime(toWire(time)))),
    uninstall: () =>
      host.act("clock.uninstall", "", async () => {
        const result = await driver().uninstall();
        scope.dropped += result.dropped;
        if (result.dropped > 0) await host.diagnostic("clock", droppedNote(result.dropped, "clock.uninstall"));
      }),
  };
}
