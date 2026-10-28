import type {
  ChromeElement,
  GestureDirection,
  GestureInput,
  GestureOptions,
  GestureResult,
  PageGesture,
  RefreshIndicator,
  RefreshPhase,
} from "@lingxia/types/automation";
import { PageLocator, sleep } from "./locator.js";
import type {
  DragOptions,
  FlingOptions,
  GestureRecord,
  GestureTarget,
  GestureTrace,
  Locator,
  PullToRefreshOptions,
  PullToRefreshResult,
  SwipeOptions,
  TestFinger,
  TouchOptions,
} from "./types.js";

type Point = [number, number];
type Act = <T>(verb: string, detail: string, op: () => Promise<T>) => Promise<T>;

/** What the gesture helpers need from a fixture. */
export interface GestureScope {
  driver: () => PageGesture;
  act: Act;
  record: (entry: GestureRecord) => void;
  /** `{ page }` of a bound page; empty for the current page. */
  target: { page?: string };
}

const POINT_TIMEOUT_MS = 5_000;
const SAMPLE_SEGMENTS = 6;
const SETTLE_MS = 1_000;

function trace(result: GestureResult): GestureTrace {
  return { input: result.input, delivered: result.delivered, events: result.events, durationMs: result.durationMs };
}

function checkPoint(value: unknown, name: string): Point {
  if (!Array.isArray(value) || value.length !== 2 || !value.every((v) => typeof v === "number" && Number.isFinite(v))) {
    throw new TypeError(`${name} must be [x, y] or a locator`);
  }
  return [value[0], value[1]];
}

/** The center of a locator's unique, on-screen match. */
async function centerOf(locator: Locator, name: string): Promise<Point> {
  if (!(locator instanceof PageLocator)) throw new TypeError(`${name} must be [x, y] or a locator from this view`);
  const until = Date.now() + POINT_TIMEOUT_MS;
  let reason = "not attached";
  for (;;) {
    const match = await locator.resolve(undefined, "gesture");
    if (match.kind === "unique" && match.rect && match.inViewport) {
      return [match.rect.left + match.rect.width / 2, match.rect.top + match.rect.height / 2];
    }
    reason = match.kind === "unique" ? "element is outside the viewport; scroll it in first" : locator.missText(match);
    if (Date.now() >= until) throw new Error(`${name}: ${locator.describe()} ${reason}`);
    await sleep(100);
  }
}

async function pointOf(target: GestureTarget, name: string): Promise<Point> {
  return Array.isArray(target) ? checkPoint(target, name) : centerOf(target as Locator, name);
}

function positive(value: number | undefined, fallback: number, name: string): number {
  const result = value ?? fallback;
  if (!Number.isFinite(result) || result < 0) throw new TypeError(`${name} must be a non-negative number`);
  return result;
}

/** Gestures on a page (`t.app.view`) or, with `window`, on the host window. */
export class Gestures {
  constructor(private readonly scope: GestureScope, private readonly window: boolean) {}

  private where(): Pick<GestureOptions, "page" | "window"> {
    return this.window ? { window: true } : { ...this.scope.target };
  }

  private async perform(api: string, options: Omit<GestureOptions, "page" | "window">): Promise<GestureResult> {
    const result = await this.scope.driver().perform({ ...options, ...this.where() });
    this.scope.record({ api, ...trace(result) });
    return result;
  }

  private async viewport(): Promise<{ left: number; top: number; width: number; height: number }> {
    return this.scope.driver().viewport(this.where());
  }

  swipe(options: SwipeOptions): Promise<GestureTrace> {
    return this.scope.act("gesture.swipe", "", async () => {
      const from = await pointOf(options.from, "swipe from");
      const to = await pointOf(options.to, "swipe to");
      return trace(await this.perform("swipe", {
        shape: {
          kind: "line", from, to,
          durationMs: positive(options.durationMs, 300, "durationMs"),
          ...(options.steps !== undefined ? { steps: options.steps } : {}),
          holdMs: positive(options.hold, 100, "hold"),
        },
        ...(options.input ? { input: options.input } : {}),
      }));
    });
  }

  drag(locator: Locator, options: DragOptions): Promise<GestureTrace> {
    return this.scope.act("gesture.drag", locator instanceof PageLocator ? locator.describe() : "", async () => {
      const from = await centerOf(locator, "drag");
      const to: Point = [from[0] + options.dx, from[1] + options.dy];
      return trace(await this.perform("drag", {
        shape: { kind: "line", from, to, durationMs: positive(options.durationMs, 500, "durationMs"), holdMs: positive(options.hold, 100, "hold") },
        // A drag is a pressed pointer: a finger, or the mouse on a desktop.
        input: options.input ?? (this.scope.driver().inputs().includes("touch") ? "touch" : "mouse"),
      }));
    });
  }

  fling(direction: GestureDirection, velocity = 3000, options: FlingOptions = {}): Promise<GestureTrace> {
    return this.scope.act("gesture.fling", `${direction} ${velocity}px/s`, async () => {
      if (!["up", "down", "left", "right"].includes(direction)) throw new TypeError("fling direction must be up, down, left or right");
      let from: Point;
      if (options.at !== undefined) from = await pointOf(options.at, "fling at");
      else {
        const v = await this.viewport();
        const fraction = { up: [0.5, 0.75], down: [0.5, 0.25], left: [0.75, 0.5], right: [0.25, 0.5] }[direction];
        from = [v.left + v.width * fraction[0], v.top + v.height * fraction[1]];
      }
      return trace(await this.perform("fling", {
        shape: { kind: "fling", from, direction, velocity },
        ...(options.input ? { input: options.input } : {}),
      }));
    });
  }

  touch(at: GestureTarget, options: TouchOptions = {}): Promise<TestFinger> {
    return this.scope.act("gesture.touch", "", async () => {
      const start = await pointOf(at, "touch at");
      const down = await this.perform("touch", {
        shape: { kind: "hold", at: start, durationMs: 0 },
        end: "hold",
        ...(options.input ? { input: options.input } : {}),
      });
      return this.finger(down, start);
    });
  }

  private finger(down: GestureResult, start: Point): TestFinger {
    let id = down.finger;
    let at = start;
    const self = this;
    const next = async (api: string, shape: GestureOptions["shape"], end: "hold" | "up" | "cancel") => {
      if (id === undefined) throw new Error("this finger is already lifted");
      const result = await self.perform(api, { shape, end, finger: id });
      id = result.finger;
      return trace(result);
    };
    return {
      get at() { return [at[0], at[1]] as [number, number]; },
      moveTo: (target, options = {}) => self.scope.act("finger.moveTo", "", async () => {
        const to = await pointOf(target, "moveTo");
        const result = await next("finger.moveTo", {
          kind: "line", from: at, to,
          durationMs: positive(options.durationMs, 200, "durationMs"),
          ...(options.steps !== undefined ? { steps: options.steps } : {}),
        }, "hold");
        at = to;
        return result;
      }),
      hold: (ms) => self.scope.act("finger.hold", `${ms}ms`, () =>
        next("finger.hold", { kind: "hold", at, durationMs: positive(ms, 0, "hold") }, "hold")),
      up: () => self.scope.act("finger.up", "", () => next("finger.up", { kind: "hold", at, durationMs: 0 }, "up")),
      cancel: () => self.scope.act("finger.cancel", "", () => next("finger.cancel", { kind: "hold", at, durationMs: 0 }, "cancel")),
    };
  }

  pageViewport(): Promise<{ left: number; top: number; width: number; height: number }> {
    return this.scope.act("view.viewport", "", () => this.viewport());
  }

  refreshIndicator(): Promise<RefreshIndicator> {
    return this.scope.act("gesture.refreshIndicator", "", () => this.scope.driver().refreshIndicator(this.scope.target));
  }

  pullToRefresh(options: PullToRefreshOptions = {}): Promise<PullToRefreshResult> {
    return this.scope.act("gesture.pullToRefresh", "", async () => {
      const distance = positive(options.distance, 160, "distance");
      const hold = positive(options.hold, 300, "hold");
      const read = () => this.scope.driver().refreshIndicator(this.scope.target);
      // A page without an indicator rejects here, before any input.
      const phases: RefreshPhase[] = [];
      let maxOffset = 0;
      const sample = async () => {
        const state = await read();
        if (phases[phases.length - 1] !== state.phase) phases.push(state.phase);
        maxOffset = Math.max(maxOffset, state.offset);
        return state;
      };
      await sample();
      let start: Point;
      if (options.at !== undefined) start = await pointOf(options.at, "pullToRefresh at");
      else {
        const v = await this.viewport();
        start = [v.left + v.width / 2, v.top + Math.min(v.height * 0.25, 120)];
      }
      const finger = await this.touch(start, options.input ? { input: options.input } : {});
      const traces: GestureTrace[] = [];
      const step = distance / SAMPLE_SEGMENTS;
      const segmentMs = Math.max(40, (distance * 2.5) / SAMPLE_SEGMENTS);
      for (let i = 1; i <= SAMPLE_SEGMENTS; i++) {
        traces.push(await finger.moveTo([start[0], start[1] + step * i], { durationMs: segmentMs }));
        await sample();
      }
      if (hold > 0) {
        traces.push(await finger.hold(hold));
        await sample();
      }
      const result: PullToRefreshResult = { refreshed: false, maxOffset, phases, input: traces[0]?.input ?? "touch",
        delivered: traces.every((t) => t.delivered === "window") ? "window" : "view",
        events: traces.reduce((n, t) => n + t.events, 0), durationMs: traces.reduce((n, t) => n + t.durationMs, 0) };
      if (options.release === false) return { ...result, finger };
      const up = await finger.up();
      result.events += up.events;
      result.durationMs += up.durationMs;
      const settleUntil = Date.now() + SETTLE_MS;
      while (Date.now() < settleUntil) {
        const state = await sample();
        if (state.phase === "refreshing") break;
        if (state.phase === "idle" && state.offset === 0) break;
        await sleep(50);
      }
      result.maxOffset = maxOffset;
      result.refreshed = phases.includes("refreshing");
      return result;
    });
  }

  chromeElements(scope?: string): Promise<ChromeElement[]> {
    return this.scope.act("chrome.elements", scope ?? "", () => this.scope.driver().chrome(scope ? { scope } : {}));
  }

  /** Wait for exactly one chrome element with the label in `scopes`, then tap its center. */
  chromeTap(label: string, scopes: readonly string[] | undefined, timeout = 5_000): Promise<GestureTrace> {
    return this.scope.act("chrome.tap", label, async () => {
      const until = Date.now() + timeout;
      let seen: ChromeElement[] = [];
      for (;;) {
        const all = await this.scope.driver().chrome({});
        seen = all.filter((e) => e.label === label && (!scopes || scopes.includes(e.scope)));
        if (seen.length === 1) break;
        if (Date.now() >= until) {
          const near = all.filter((e) => !scopes || scopes.includes(e.scope)).map((e) => `${e.scope}:${JSON.stringify(e.label)}`);
          throw new Error(`${seen.length} chrome elements are labelled ${JSON.stringify(label)}${scopes ? ` in ${scopes.join("/")}` : ""}; present: ${near.slice(0, 20).join(", ") || "none"}`);
        }
        await sleep(150);
      }
      const [x, y, w, h] = seen[0]!.bounds;
      const at: Point = [x + w / 2, y + h / 2];
      const input = this.scope.driver().inputs().includes("touch") ? "touch" : "mouse";
      const result = await this.scope.driver().perform({
        shape: { kind: "line", from: at, to: at, durationMs: 50, holdMs: 0 }, input, window: true,
      });
      this.scope.record({ api: "chrome.tap", ...trace(result) });
      return trace(result);
    });
  }

  edgeBack(options: { edge?: "left" | "right" } = {}): Promise<GestureTrace> {
    return this.scope.act("gesture.edgeBack", options.edge ?? "left", async () => {
      const result = await this.scope.driver().edgeBack(options);
      const out = trace(result as GestureResult);
      this.scope.record({ api: "edgeBack", ...out });
      return out;
    });
  }
}

/** Gesture devices of this host; empty when it has none (or no automation). */
export function hostInputs(gesture: () => PageGesture | undefined): GestureInput[] {
  try {
    return gesture()?.inputs() ?? [];
  } catch {
    return [];
  }
}
