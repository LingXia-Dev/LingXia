/**
 * The contract between this package and the host that runs it (lxdev's test
 * runtime). Not part of the authoring surface: spec code never sees these.
 */
import type { ProtocolReport } from "./report-types.js";
export interface LingxiaTestController {
  run(): Promise<ProtocolReport>;
  /** The specs `run()` would run, in `listed`, without running them. */
  list(): Promise<ProtocolReport>;
  readonly version: string;
  /** Clears the registry. Used by this package's own Node tests. */
  reset(): void;
}

export interface AutomationHost {
  args?: Record<string, string>;
  /** lxdev run controls, kept apart from the user's `args`. */
  control?: Record<string, string>;
  attach?: (
    name: string,
    artifact: { mimeType: string; base64: string },
  ) => void | Promise<void>;
  emit?: (event: Record<string, unknown>) => void | Promise<void>;
  report?: (event: Record<string, unknown>) => void | Promise<void>;
  logs?: () => string | string[] | Promise<string | string[]>;
  /** Logic network calls since `sinceMs`, newest `limit`. */
  networkLog?: (sinceMs: number, limit?: number) => unknown;
  /** `lxdev test --record-network`: start, or stop and return the scenario. */
  networkRecord?: (command: "start" | "stop", name?: string) => unknown;
  /** Whether the screen is locked; `undefined` where the host cannot tell. */
  screenLocked?: () => boolean | undefined;
  /**
   * Open a spec attempt: routes, mock scenarios and test clocks installed
   * from now on belong to it. Returns its token.
   */
  beginAttempt?: () => number;
  /**
   * Close the attempt: installs are refused until the next one opens, and
   * everything the attempt installed is removed. Rejects when something
   * could not be removed.
   */
  endAttempt?: (token: number) => Promise<AttemptReclaim>;
  /**
   * Refuse every further driver call from this run's context and remove
   * what the open attempt installed: the run stops running spec code.
   */
  revoke?: (reason: string) => Promise<AttemptReclaim>;
}

/** What the host removed when an attempt closed. */
export interface AttemptReclaim {
  routes: number;
  scenarios: number;
  clocks: number;
  /** Test timers pending on the removed clocks; they never fired. */
  droppedTimers: number;
  /** Dialog watches the attempt left open. */
  dialogs?: number;
}

declare global {
  // eslint-disable-next-line no-var
  var __LINGXIA_TEST__: LingxiaTestController | undefined;
  // eslint-disable-next-line no-var
  var __LINGXIA_AUTOMATION_HOST__: AutomationHost | undefined;
  // eslint-disable-next-line no-var
  var __RONG_TEST_HOST__: AutomationHost | undefined;
  // eslint-disable-next-line no-var
  var __LINGXIA_TEST_SOURCE_MAP__: unknown;
  // eslint-disable-next-line no-var
  var __LINGXIA_CLI_VERSION__: string | undefined;
}
