/**
 * The shapes of `report.json` and of the run protocol lxdev reads, published
 * as `@lingxia/test/report` for tools that read a report.
 */

export type SpecStatus =
  | "passed"
  | "failed"
  | "skipped"
  | "timeout"
  | "xfail"
  | "xpass";

export type StepStatus = "passed" | "failed" | "timeout" | "skipped";

export interface AssertionRecord {
  matcher: string;
  expected: string;
  actual: string;
  passed: boolean;
  step?: string;
}

export interface StepRecord {
  name: string;
  path: string;
  /** `step` is authored with `t.step`; `action` is a recorded driver call. */
  kind?: "step" | "action";
  /** Short argument summary for an action — a selector, a page, a script head. */
  detail?: string;
  /** Identical consecutive actions collapse into one row with a count. */
  repeat?: number;
  status: StepStatus;
  duration_ms: number;
  error?: ReportError;
  steps: StepRecord[];
  attachments: AttachmentRef[];
  assertions: AssertionRecord[];
}

export interface AttachmentRef {
  name: string;
  path: string;
  mimeType: string;
}

/** A page instance, as a failure names it. */
export interface FailurePage {
  name: string | null;
  instanceId: string | null;
  /**
   * Set when the page was hidden at the failure (window covered, minimized
   * or display asleep): WebKit pauses animation frames, so rAF- and
   * transition-driven UI never moved.
   */
  hidden?: string;
}

/** `document.visibilityState` and whether an animation frame arrived. */
export interface PageVisibility {
  state: string;
  animationFrames: boolean;
  /** The explanation, when the page was hidden or got no frame. */
  note?: string;
}

/**
 * One Logic `fetch` (or `Rong.SSE` connection) the app made, as a failed
 * spec's report lists it: no bodies, no headers, credentials and
 * `--secret-arg` values masked in the URL.
 */
export interface FailureNetworkCall {
  /** Epoch milliseconds when the request started. */
  time: number;
  /** `function`: a Worker Function call the dev session's companion saw. */
  kind: "fetch" | "sse" | "function";
  /** Empty for a `function` call. */
  method: string;
  /** Empty for a `function` call. */
  url: string;
  /** `function` calls: the Function and `result`/`error`/`fault`/`default`. */
  function?: string;
  outcome?: string;
  /** Response status; `null` when it failed or has not answered. */
  status: number | null;
  /** The rejection, e.g. `TypeError: fetch failed`. */
  error?: string;
  durationMs: number | null;
  /** `route` when a test route answered; `network` when the request was real. */
  source: "route" | "network";
  /** The route that matched, including a `continue` pass-through. */
  route?: { pattern: string; action: string; rule?: number; scenario?: string };
  /** `rule 2 (name:variant)`, `route <pattern>`, `real`, or `companion default`. */
  answeredBy?: string;
  /** Why no scenario rule answered, when rules targeted the call. */
  noMatch?: string;
}

/** The scenario a failed spec had installed, and what each rule answered. */
export interface ScenarioReport {
  /** `name:variant`. */
  label: string;
  name: string | null;
  variant: string | null;
  rules: { index: number; target: string; kind: "http" | "function"; hits: number }[];
}

export interface ReportError {
  code?: string;
  data?: unknown;
  phase?: string;
  /** The app's last Logic network calls before the failure, oldest first. */
  network?: FailureNetworkCall[];
  /** The scenario installed with `t.scenario.use()` when the spec failed. */
  scenario?: ScenarioReport;
  /** The recorded driver action that failed, e.g. `page.click [data-testid=save]`. */
  failedAction?: string;
  /** The page that was current when the spec failed. */
  page?: FailurePage;
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
  attempt?: number;
  attempts?: CaseRecord[];
  flaky?: boolean;
  id: string;
  title: string;
  name: string;
  full_name: string;
  /** Which `--repeat-each` execution this is (1-based); absent without it. */
  repeat?: number;
  /** Source file the spec was registered from, remapped through the bundle map. */
  file?: string;
  line?: number;
  /** Display group in the report — the spec file's path inside the project. */
  suite?: string;
  status: SpecStatus;
  duration_ms: number;
  covers: string[];
  /** Selection tags: the file's `spec.configure` tags, then the spec's own. */
  tags?: string[];
  /**
   * `lxdev test --openapi`: this spec's Logic `fetch` responses checked
   * against the contract. `violations` (routed responses) failed the spec;
   * `warnings` (the real server's) did not.
   */
  contract?: CaseContract;
  /**
   * `lx.*` members the spec's evals actually reached, observed by the runtime
   * rather than declared. A `covers` tag absent from here was claimed but never
   * exercised.
   */
  observed?: string[];
  steps: StepRecord[];
  assertions: AssertionRecord[];
  attachments: AttachmentRef[];
  error?: ReportError;
  timeout_ms: number;
  reason?: string;
}

/** One response that breaks the OpenAPI contract. */
export interface ContractIssue {
  /** The spec id. */
  case: string;
  /** `route`/`patch`: a route shaped it; `network`: the real server. */
  source: "route" | "patch" | "network";
  method: string;
  /** Scheme, host and path only. */
  url: string;
  status: number;
  /** `METHOD /path/{template}` as the document names it. */
  operation: string;
  /** The response schema, as a document pointer. */
  schema: string;
  /** The route that fulfilled or patched it. */
  pattern?: string;
  issues: Array<{ path: string; message: string; schema: string }>;
}

export interface CaseContract {
  /** Responses captured during the spec. */
  checked: number;
  violations: ContractIssue[];
  warnings: ContractIssue[];
}

/** `report.openapi`: what `--openapi` checked across the run. */
export interface OpenApiSummary {
  documents: Array<{ name: string; version: string; title?: string; operations: number }>;
  /** Captured responses. */
  responses: number;
  /** Responses with a documented JSON schema and a body, validated. */
  validated: number;
  routed: { validated: number; failed: number };
  network: { validated: number; mismatched: number };
  /** Matched an operation but not validated, by why. */
  skipped: { no_schema: number; not_json: number; empty: number; truncated: number };
  /** A status the operation does not document. */
  undocumented: Array<{ operation: string; status: number; source: string; count: number }>;
  /** Requests no operation describes (another API, or a gap in the document). */
  unmatched: Array<{ method: string; path: string; count: number }>;
  /** Real-server mismatches (at most 100). */
  warnings: ContractIssue[];
}

/** `report.tag_summary`: one row per tag, and `(untagged)` for the rest. */
export interface TagSummary {
  tag: string;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  timeout: number;
  xfail: number;
  xpass: number;
  flaky: number;
  /** No failed, timed-out or xpass spec carries the tag. */
  ok: boolean;
}

/** A spec covering a manifest id, with its outcome in this run. */
export interface CoverageSpec {
  id: string;
  title: string;
  /** `not_run`: registered, but outside this selection. */
  status: SpecStatus | "not_run";
}

/** `report.coverage`: the `--covers-manifest` ids against the suite's `covers`. */
export interface CoverageSummary {
  total: number;
  /** Ids at least one registered spec covers. */
  covered: number;
  /** Covered ids whose specs passed (none broke). */
  passing: number;
  /** Covered ids with a failed, timed-out or xpass spec. */
  failing: number;
  /** Manifest ids no spec covers. */
  uncovered: Array<{ id: string; title?: string }>;
  /** `covers` ids used by specs but missing from the manifest. */
  unknown: Array<{ id: string; specs: string[] }>;
  ids: Array<{
    id: string;
    title?: string;
    status: "passed" | "failed" | "xfail" | "skipped" | "not_run" | "uncovered";
    specs: CoverageSpec[];
  }>;
}

/** The app under test, so a report identifies its own subject. */
export interface RunSubject {
  appid?: string;
  app_name?: string;
  version?: string;
  release_type?: string;
  pages?: number;
}

export interface RunMeta {
  started_at: string;
  duration_ms: number;
  /** User args; declared secrets and credential-named keys are `***`. */
  args: Record<string, string>;
  /** lxdev's run controls (grep, ids, shard, retries, …). */
  run?: Record<string, string>;
  platform?: string;
  framework?: string;
  subject?: RunSubject;
  /** The suite opted into measuring the whole published `lx` surface. */
  surface_coverage?: boolean;
  /**
   * The whole-run budget: fixed by `--timeout-secs`, or scaled to the
   * planned executions (`auto`). `exhausted_after` counts the specs that
   * finished before it ran out; the rest are skipped as not run.
   */
  budget?: { ms: number; auto: boolean; planned: number; exhausted_after?: number };
  /** `--shuffle` seed; rerun with `--shuffle=<seed>` for the same order. */
  shuffle_seed?: number;
  /** `--repeat-each` count. */
  repeat_each?: number;
}

/** One failed case, flat, for tools that only need what broke and where. */
export interface FailureRecord {
  id: string;
  title: string;
  file?: string;
  line?: number;
  phase?: string;
  code?: string;
  message: string;
  failedAction?: string;
  page?: FailurePage;
  /** Report-relative path of the failure screenshot, when one was captured. */
  screenshot?: string;
  /** The app's last Logic network calls before the failure (up to 20). */
  network?: FailureNetworkCall[];
  /** The scenario installed when the spec failed. */
  scenario?: ScenarioReport;
}

export interface JsonReport {
  schema_version?: number;
  framework: { name: string; version: string };
  meta: RunMeta;
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
  /** Every failed, timed-out or xpass case, flattened from `cases`. */
  failures?: FailureRecord[];
  /** Per-tag outcome, when any spec is tagged. */
  tag_summary?: TagSummary[];
  /** `--covers-manifest` summary. */
  coverage?: CoverageSummary;
  /** `--openapi` contract summary. */
  openapi?: OpenApiSummary;
  /** `lxdev test --list`: the selected specs; nothing ran. */
  listed?: ListedSpec[];
}

/** A spec `lxdev test --list` shows. */
export interface ListedSpec {
  id: string;
  title: string;
  file: string;
  line: number;
  tags: string[];
}

export type ProtocolReport = JsonReport;
