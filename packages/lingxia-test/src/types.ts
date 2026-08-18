import type { LxAppDriver, PageDriver } from "@lingxia/types/automation";

export type SpecStatus =
  | "passed"
  | "failed"
  | "skipped"
  | "timeout"
  | "xfail"
  | "xpass";

export type StepStatus = "passed" | "failed" | "timeout";

export interface SpecOptions {
  /** Stable id. ASCII titles slug by default; non-ASCII titles need this or become `file-n`. */
  id?: string;
  /** Declared coverage tags. Journeys omit this. */
  covers?: readonly string[];
  /** Spec budget in ms (default 30_000). */
  timeout?: number;
  /** Relaunch the home page before the body. */
  fresh?: boolean;
  /** Pin `t.app` to this lxapp id instead of the current one. */
  app?: string;
  /** Skip auto-attached failure forensics (only when capture itself would wedge). */
  forensics?: boolean;
}

export type SpecBody = (t: Fixture) => void | Promise<void>;

export interface ExpectOptions {
  timeout?: number;
  interval?: number;
}

export interface RejectExpected {
  code?: string;
  message?: string | RegExp;
}

export interface Locator {
  readonly selector: string;
  click(options?: ExpectOptions): Promise<void>;
  fill(text: string, options?: ExpectOptions): Promise<void>;
  type(text: string, options?: ExpectOptions): Promise<void>;
  query(options?: ExpectOptions): Promise<unknown>;
}

export interface TestPage extends PageDriver {
  testId(id: string): Locator;
  css(selector: string): Locator;
}

export interface TestApp extends LxAppDriver {
  readonly page: TestPage;
}

export interface Apps {
  lxapp(appId: string): TestApp;
}

export interface RetryMatchers<T> {
  readonly not: RetryMatchers<T>;
  toBe(expected: unknown): Promise<void>;
  toEqual(expected: unknown): Promise<void>;
  toContain(expected: unknown): Promise<void>;
  toMatch(expected: string | RegExp): Promise<void>;
  toBeTruthy(): Promise<void>;
  toBeFalsy(): Promise<void>;
  toBeDefined(): Promise<void>;
  toBeUndefined(): Promise<void>;
  toBeInstanceOf(expected: Function): Promise<void>;
}

export interface LocatorMatchers {
  readonly not: LocatorMatchers;
  toBeVisible(options?: ExpectOptions): Promise<void>;
  toHaveText(expected: string | RegExp, options?: ExpectOptions): Promise<void>;
  toHaveCount(expected: number, options?: ExpectOptions): Promise<void>;
  toHaveValue(expected: string | RegExp, options?: ExpectOptions): Promise<void>;
}

export interface FixtureExpect {
  (locator: Locator): LocatorMatchers;
  poll<T>(read: () => T | Promise<T>, options?: ExpectOptions): RetryMatchers<T>;
}

export interface Fixture {
  readonly app: TestApp;
  readonly apps: Apps;
  readonly args: Record<string, string>;
  step<T>(name: string, body: () => T | Promise<T>): Promise<T>;
  expect: FixtureExpect;
  reject(
    operation: () => unknown | Promise<unknown>,
    expected?: RejectExpected,
  ): Promise<unknown>;
  defer(cleanup: () => void | Promise<void>): void;
  attach(name: string, data: unknown): Promise<void>;
}

export interface Matchers<T> {
  readonly not: Matchers<T>;
  toBe(expected: unknown): void;
  toEqual(expected: unknown): void;
  toContain(expected: unknown): void;
  toMatch(expected: string | RegExp): void;
  toBeTruthy(): void;
  toBeFalsy(): void;
  toBeDefined(): void;
  toBeUndefined(): void;
  toBeInstanceOf(expected: Function): void;
  toThrow(expected?: unknown): void;
}

export interface SourceLocation {
  source: string;
  line: number;
  column: number;
}

export interface StepRecord {
  name: string;
  path: string;
  status: StepStatus;
  duration_ms: number;
  error?: ReportError;
  steps: StepRecord[];
  attachments: AttachmentRef[];
}

export interface AttachmentRef {
  name: string;
  path: string;
  mimeType: string;
}

export interface ReportError {
  name: string;
  message: string;
  stack?: string;
  matcher?: string;
  expected?: string;
  actual?: string;
  location?: string;
  step?: string;
}

export interface CaseRecord {
  id: string;
  title: string;
  name: string;
  full_name: string;
  status: SpecStatus;
  duration_ms: number;
  covers: string[];
  steps: StepRecord[];
  attachments: AttachmentRef[];
  error?: ReportError;
  timeout_ms: number;
}

export interface JsonReport {
  framework: { name: string; version: string };
  partial: boolean;
  filtered: boolean;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  xfail: number;
  xpass: number;
  timeout: number;
  duration_ms: number;
  cases: CaseRecord[];
}

export interface ProtocolCase {
  name: string;
  full_name: string;
  status: "passed" | "failed" | "skipped";
  duration_ms: number;
  error?: {
    name: string;
    message: string;
    stack?: string;
    causes?: unknown[];
  };
}

export interface ProtocolReport {
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  duration_ms: number;
  cases: ProtocolCase[];
}

export interface LingxiaTestController {
  run(): Promise<ProtocolReport>;
  readonly version: string;
  /** Clears the registry. Used by this package's own Node tests. */
  reset(): void;
}

export interface AutomationHost {
  args?: Record<string, string> | unknown;
  attach?: (
    name: string,
    artifact: { mimeType: string; base64: string },
  ) => void | Promise<void>;
  emit?: (event: Record<string, unknown>) => void | Promise<void>;
  report?: (event: Record<string, unknown>) => void | Promise<void>;
  logs?: () => string | string[] | Promise<string | string[]>;
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
  // eslint-disable-next-line no-var
  var lx:
    | {
        automation: () => {
          lxapp: {
            (): LxAppDriver;
            (appid: string): LxAppDriver;
          };
        };
      }
    | undefined;
}
