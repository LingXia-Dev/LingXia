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
  return `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="${attr(PACKAGE_NAME)}" tests="${report.total}" failures="${failures}" errors="0" skipped="${report.skipped}" time="${seconds(report.duration_ms)}">
${body}</testsuites>
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
  if (item.status === "xfail") {
    inner.push(`      <system-out>${text("spec.fail: failed as declared")}</system-out>\n`);
  }
  if (item.covers.length > 0) {
    inner.push(`      <properties>\n        <property name="covers" value="${attr(item.covers.join(" "))}"/>\n      </properties>\n`);
  }
  return inner.length === 0
    ? `    <testcase ${attrs}/>\n`
    : `    <testcase ${attrs}>\n${inner.join("")}    </testcase>\n`;
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
