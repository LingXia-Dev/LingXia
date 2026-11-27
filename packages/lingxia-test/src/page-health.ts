import type { HostRunAutomation, PageEvent } from "@lingxia/types/automation";
import type { CasePageHealth, PageHealthEvent } from "./report-types.js";
import type { PageFailureMatch, PageHealthOptions } from "./types.js";

/** Bounds each read of the host's page events. */
export const PAGE_EVENTS_CALL_MS = 5_000;

const CHECKS = ["events", "frameGap", "blank"] as const;
type Check = typeof CHECKS[number];

const OPT_OUT_KIND: Partial<Record<PageEvent["kind"], Check>> = { frame_gap: "frameGap", blank_page: "blank" };

/** Throws on a malformed `expectPageFailure` or `pageHealth`. */
export function validatePageHealthOptions(
  expect: unknown,
  health: unknown,
  where: string,
): void {
  if (expect !== undefined && !(expect instanceof RegExp) && !(typeof expect === "string" && expect.length > 0)) {
    const ok = expect !== null && typeof expect === "object" && !Array.isArray(expect)
      && Object.keys(expect).every((key) => key === "code" || key === "page")
      && ((expect as { code?: unknown }).code !== undefined || (expect as { page?: unknown }).page !== undefined)
      && ["code", "page"].every((key) => {
        const value = (expect as Record<string, unknown>)[key];
        return value === undefined || value instanceof RegExp || (typeof value === "string" && value.length > 0);
      });
    if (!ok) throw new TypeError(`${where} expectPageFailure must be a RegExp, a code, or { code?, page? }`);
  }
  if (health !== undefined) {
    const ok = health !== null && typeof health === "object" && !Array.isArray(health)
      && Object.entries(health).every(([key, value]) => (key === "frameGap" || key === "blank") && value === false);
    if (!ok) throw new TypeError(`${where} pageHealth takes only { frameGap?: false, blank?: false }`);
  }
}

function test(pattern: RegExp | string, value: string | undefined): boolean {
  if (value === undefined) return false;
  if (typeof pattern === "string") return pattern === value;
  pattern.lastIndex = 0;
  return pattern.test(value);
}

/** Whether `event` is the failure the test declared. */
export function matchesPageFailure(match: PageFailureMatch, event: PageEvent): boolean {
  if (match instanceof RegExp) return test(match, `${event.kind} ${event.code ?? ""} ${event.message}`);
  if (typeof match === "string") return event.code === match;
  if (match.code !== undefined && !test(match.code, event.code)) return false;
  if (match.page !== undefined && !test(match.page, event.page)) return false;
  return true;
}

function describeMatch(match: PageFailureMatch): string {
  if (match instanceof RegExp || typeof match === "string") return String(match);
  return JSON.stringify({ code: match.code?.toString(), page: match.page?.toString() });
}

export function describeEvent(event: PageEvent | PageHealthEvent): string {
  const where = [("appId" in event ? event.appId : event.app_id), event.page, event.surface].filter(Boolean).join(" ");
  const code = event.code ? ` ${event.code}` : "";
  return `${event.kind}${code} on ${where}: ${event.message}`;
}

/** A page problem the test did not declare. */
export class PageHealthError extends Error {
  override readonly name = "PageHealthError";
  readonly code = "E_PAGE_HEALTH";
}

/** Reads the host's run-wide page events from where the last read stopped. */
export class PageEventReader {
  private seq = 0;
  /** `false` once the host answered that it has no page events. */
  available = true;

  reset(): void {
    this.seq = 0;
    this.available = true;
  }

  async read(root: () => HostRunAutomation): Promise<{ events: PageEvent[]; dropped: number } | undefined> {
    if (!this.available) return undefined;
    const driver = (() => {
      try { return root().pageEvents; } catch { return undefined; }
    })();
    if (!driver || typeof driver.read !== "function") {
      this.available = false;
      return undefined;
    }
    let handle: unknown;
    try {
      const result = await Promise.race([
        Promise.resolve(driver.read({ since: this.seq })),
        new Promise<never>((_, reject) => {
          handle = setTimeout(() => reject(new Error("reading page events timed out")), PAGE_EVENTS_CALL_MS);
        }),
      ]);
      for (const event of result.events) this.seq = Math.max(this.seq, event.seq);
      return { events: result.events, dropped: result.dropped ?? 0 };
    } finally {
      clearTimeout(handle as ReturnType<typeof setTimeout>);
    }
  }
}

/**
 * Grades one attempt's page events. Undeclared events fail it; with
 * `expectPageFailure`, every event must match and one must occur. Events
 * of an opted-out check are recorded, not graded.
 */
export function gradePageHealth(
  read: { events: PageEvent[]; dropped: number } | undefined,
  expect: PageFailureMatch | undefined,
  health: PageHealthOptions | undefined,
): { record: CasePageHealth; error?: PageHealthError } {
  const optedOut = CHECKS.filter((check) => health?.[check as keyof PageHealthOptions] === false);
  const events = read?.events ?? [];
  const graded: PageHealthEvent[] = [];
  const unexpected: PageEvent[] = [];
  let matched = 0;
  for (const event of events) {
    const optOut = OPT_OUT_KIND[event.kind];
    const skipped = optOut !== undefined && optedOut.includes(optOut);
    const expected = !skipped && expect !== undefined && matchesPageFailure(expect, event);
    if (expected) matched += 1;
    else if (!skipped) unexpected.push(event);
    graded.push({
      seq: event.seq,
      run_ms: event.atMs,
      app_id: event.appId,
      ...(event.page !== undefined ? { page: event.page } : {}),
      ...(event.instance !== undefined ? { instance: event.instance } : {}),
      ...(event.surface !== undefined ? { surface: event.surface } : {}),
      kind: event.kind,
      ...(event.code !== undefined ? { code: event.code } : {}),
      message: event.message,
      ...(event.gapMs !== undefined ? { gap_ms: event.gapMs } : {}),
      expected,
      ...(skipped ? { opted_out: true } : {}),
    });
  }
  const record: CasePageHealth = {
    events: graded,
    dropped: read?.dropped ?? 0,
    checks: read ? CHECKS.filter((check) => !optedOut.includes(check)) : [],
    opted_out: optedOut,
    ...(read ? {} : { unavailable: true }),
  };
  if (!read) return { record };
  const lines: string[] = [];
  if (unexpected.length > 0) {
    const head = unexpected.length === 1 ? "A page failed during the test:" : `${unexpected.length} page failures during the test:`;
    lines.push(head, ...unexpected.slice(0, 5).map((event) => `  ${describeEvent(event)}`));
    if (unexpected.length > 5) lines.push(`  …and ${unexpected.length - 5} more`);
    if (expect !== undefined) lines.push(`expectPageFailure ${describeMatch(expect)} does not match them.`);
  } else if (expect !== undefined && matched === 0) {
    lines.push(`expectPageFailure ${describeMatch(expect)}: no page failed during the test.`);
  }
  // Hundreds of events in one test are a failure even if the lost ones matched.
  if (record.dropped > 0) lines.push(`${record.dropped} page events were lost to the host's bound.`);
  if (lines.length === 0) return { record };
  return { record, error: new PageHealthError(lines.join("\n")) };
}
