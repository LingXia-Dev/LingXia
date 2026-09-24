import type { CaseRecord, JsonReport } from "./types.js";
import { PACKAGE_NAME } from "./version.js";

/**
 * JUnit XML is what CI dashboards (GitHub, GitLab, Jenkins, Azure) ingest, so
 * every run emits one next to the HTML report. `xfail` counts as a pass and
 * `xpass` as a failure, matching how the run itself is graded.
 */
export function renderJUnit(report: JsonReport): string {
  const suites = new Map<string, CaseRecord[]>();
  for (const item of report.cases) {
    const name = item.suite ?? "specs";
    const bucket = suites.get(name) ?? [];
    bucket.push(item);
    suites.set(name, bucket);
  }
  const timestamp = report.meta?.started_at || new Date(0).toISOString();
  const body = [...suites.entries()]
    .map(([name, cases]) => renderSuite(name, cases, timestamp))
    .join("");
  const failures = report.failed + report.timeout + report.xpass;
  const errors = report.partial ? 1 : 0;
  const interrupted = report.partial
    ? '  <testsuite name="runner" tests="1" failures="0" errors="1"><testcase name="incomplete run"><error message="The test run did not finish"/></testcase></testsuite>\n'
    : "";
  return `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="${attr(PACKAGE_NAME)}" tests="${report.total + errors}" failures="${failures}" errors="${errors}" skipped="${report.skipped}" time="${seconds(report.duration_ms)}">
${renderRunProperties(report)}${body}${interrupted}</testsuites>
`;
}

function renderSuite(name: string, cases: CaseRecord[], timestamp: string): string {
  const failures = cases.filter((item) => graded(item) === "failure").length;
  const skipped = cases.filter((item) => graded(item) === "skipped").length;
  const time = cases.reduce((sum, item) => sum + item.duration_ms, 0);
  const body = cases.map((item) => renderCase(item, name)).join("");
  return `  <testsuite name="${attr(name)}" tests="${cases.length}" failures="${failures}" errors="0" skipped="${skipped}" time="${seconds(time)}" timestamp="${attr(timestamp)}">
${body}  </testsuite>
`;
}

function renderCase(item: CaseRecord, suite: string): string {
  const attrs = [
    `name="${attr(item.title)}"`,
    `classname="${attr(suite)}"`,
    `time="${seconds(item.duration_ms)}"`,
    item.file ? `file="${attr(item.file)}"` : "",
    item.line ? `line="${item.line}"` : "",
  ].filter(Boolean).join(" ");
  const verdict = graded(item);
  const inner: string[] = [];
  if (verdict === "skipped") {
    inner.push(`      <skipped message="${attr(item.reason ?? "pending")}"/>\n`);
  } else if (verdict === "failure") {
    const error = item.error;
    const detail = error
      ? `${error.message}${error.stack ? `\n\n${error.stack}` : ""}`
      : `spec finished as ${item.status}`;
    inner.push(
      `      <failure message="${attr(firstLine(error?.message ?? item.status))}" type="${attr(error?.name ?? "Error")}">${text(detail)}</failure>\n`,
    );
  }
  if (item.flaky) { inner.push(`      <system-out>${text(`Flaky: passed after ${item.attempts?.length ?? 1} attempts`)}</system-out>\n`); }
  if (item.status === "xfail") {
    inner.push(`      <system-out>${text("spec.fail: failed as declared")}</system-out>\n`);
  }
  const properties = [
    ...(item.covers.length > 0 ? [`        <property name="covers" value="${attr(item.covers.join(" "))}"/>\n`] : []),
    ...(item.tags && item.tags.length > 0 ? [`        <property name="tags" value="${attr(item.tags.join(" "))}"/>\n`] : []),
  ];
  if (properties.length > 0) {
    inner.push(`      <properties>\n${properties.join("")}      </properties>\n`);
  }
  return inner.length === 0
    ? `    <testcase ${attrs}/>\n`
    : `    <testcase ${attrs}>\n${inner.join("")}    </testcase>\n`;
}

/**
 * Run-wide summaries as properties of the root: one per tag
 * (`tag:<name>` = `passed=3 failed=1 …`), plus coverage and contract totals.
 * Consumers that do not read root properties ignore them.
 */
function renderRunProperties(report: JsonReport): string {
  const properties: string[] = [];
  for (const row of report.tag_summary ?? []) {
    const counts = (["passed", "failed", "timeout", "xpass", "xfail", "skipped"] as const)
      .map((status) => `${status}=${row[status]}`).join(" ");
    properties.push(`<property name="tag:${attr(row.tag)}" value="${attr(`total=${row.total} ${counts}`)}"/>`);
  }
  if (report.coverage) {
    const coverage = report.coverage;
    properties.push(`<property name="coverage" value="${attr(`covered=${coverage.covered}/${coverage.total} passing=${coverage.passing} failing=${coverage.failing} uncovered=${coverage.uncovered.length} unknown=${coverage.unknown.length}`)}"/>`);
  }
  if (report.openapi) {
    const openapi = report.openapi;
    properties.push(`<property name="openapi" value="${attr(`validated=${openapi.validated} routed_failed=${openapi.routed.failed} server_mismatched=${openapi.network.mismatched} unmatched=${openapi.unmatched.reduce((sum, entry) => sum + entry.count, 0)}`)}"/>`);
  }
  if (properties.length === 0) return "";
  return `  <properties>\n${properties.map((line) => `    ${line}\n`).join("")}  </properties>\n`;
}

function graded(item: CaseRecord): "success" | "failure" | "skipped" {
  if (item.status === "skipped") return "skipped";
  if (item.status === "passed" || item.status === "xfail") return "success";
  return "failure";
}

function seconds(ms: number): string {
  return (ms / 1000).toFixed(3);
}

function firstLine(value: string): string {
  const cut = value.indexOf("\n");
  return cut === -1 ? value : value.slice(0, cut);
}

/** XML 1.0 forbids most control characters outright — drop them, don't escape. */
function scrub(value: string): string {
  let out = "";
  for (const ch of value) {
    const code = ch.codePointAt(0)!;
    if (code === 0x09 || code === 0x0a || code === 0x0d || code >= 0x20) out += ch;
  }
  return out;
}

function text(value: string): string {
  return scrub(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function attr(value: string): string {
  return text(value).replace(/"/g, "&quot;").replace(/\n/g, "&#10;");
}
