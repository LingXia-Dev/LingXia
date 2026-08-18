import { escapeHtml } from "./format.js";
import type { AssertionRecord, CaseRecord, JsonReport, SpecStatus, StepRecord } from "./types.js";
import { PACKAGE_NAME, VERSION } from "./version.js";

export function renderHtml(report: JsonReport): string {
  const cases = report.cases.map(renderCase).join("");
  const flags = [
    report.partial ? "partial" : "",
    report.filtered ? "filtered" : "",
  ].filter(Boolean).join(", ");
  const meta = report.meta ?? {
    started_at: "",
    duration_ms: report.duration_ms,
    args: {},
  };
  const argPairs = Object.entries(meta.args ?? {});
  const argHtml = argPairs.length > 0
    ? argPairs.map(([key, value]) => `<span class="kv"><dt>${escapeHtml(key)}</dt><dd>${escapeHtml(value)}</dd></span>`).join("")
    : `<span class="kv"><dt>args</dt><dd>(none)</dd></span>`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>lxdev test report</title>
<style>
:root { color-scheme: light dark; --bg:#0f1419; --card:#1a222c; --ink:#e7ecf1; --muted:#8b98a5; --pass:#3dd68c; --fail:#ff6b6b; --skip:#f5c542; --line:#2a3542; }
@media (prefers-color-scheme: light) {
  :root { --bg:#f4f6f8; --card:#fff; --ink:#15202b; --muted:#5b6773; --line:#e2e8ee; }
}
* { box-sizing: border-box; }
body { margin: 0; font: 14px/1.5 ui-sans-serif, system-ui, sans-serif; background: var(--bg); color: var(--ink); }
main { max-width: 1100px; margin: 0 auto; padding: 32px 20px 64px; }
h1 { font-size: 22px; margin: 0 0 8px; }
.lede { color: var(--muted); margin: 0 0 16px; }
dl.meta-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 8px 16px; margin: 0 0 20px; }
.kv { display: block; background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 8px 10px; }
.kv dt { font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
.kv dd { margin: 2px 0 0; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 13px; }
.summary { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 16px; }
.pill { padding: 4px 10px; border-radius: 999px; background: var(--card); border: 1px solid var(--line); cursor: pointer; }
.pill[aria-pressed="true"] { outline: 2px solid var(--ink); }
.pass { color: var(--pass); }
.fail { color: var(--fail); }
.skip { color: var(--skip); }
.filters { margin: 0 0 20px; }
.filters input { width: min(100%, 360px); padding: 8px 10px; border-radius: 8px; border: 1px solid var(--line); background: var(--card); color: var(--ink); }
.case { background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 0; margin-bottom: 12px; }
.case summary { list-style: none; cursor: pointer; padding: 14px 18px; }
.case summary::-webkit-details-marker { display: none; }
.case .body { padding: 0 18px 16px; }
.case h2 { font-size: 16px; margin: 0 0 4px; }
.meta { color: var(--muted); font-size: 13px; }
.covers { display: flex; flex-wrap: wrap; gap: 6px; margin: 8px 0; }
.tag { font-size: 12px; padding: 2px 8px; border-radius: 999px; background: var(--bg); color: var(--muted); }
.reason { margin: 8px 0 0; padding: 8px 10px; border-left: 3px solid var(--skip); background: var(--bg); }
.compare { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin: 10px 0 0; }
.compare h3 { font-size: 12px; margin: 0 0 4px; color: var(--muted); text-transform: uppercase; }
.steps { margin: 10px 0 0; padding: 0; list-style: none; }
.steps ul { margin: 6px 0 0 16px; padding: 0; list-style: none; }
.step { margin: 6px 0; }
table.assertions { width: 100%; border-collapse: collapse; margin-top: 10px; font-size: 13px; }
table.assertions th, table.assertions td { text-align: left; vertical-align: top; padding: 6px 8px; border-bottom: 1px solid var(--line); }
table.assertions th { color: var(--muted); font-weight: 600; font-size: 11px; text-transform: uppercase; }
table.assertions td { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; word-break: break-word; }
.empty { color: var(--muted); margin: 10px 0 0; }
pre { white-space: pre-wrap; background: var(--bg); padding: 10px 12px; border-radius: 8px; overflow: auto; margin: 8px 0 0; }
img { max-width: 100%; border-radius: 8px; margin-top: 8px; border: 1px solid var(--line); }
.attachments { margin-top: 8px; color: var(--muted); font-size: 12px; }
</style>
</head>
<body>
<main>
  <h1>lxdev test report</h1>
  <p class="lede">${escapeHtml(PACKAGE_NAME)} ${escapeHtml(VERSION)}${flags ? ` &middot; ${escapeHtml(flags)}` : ""} &middot; ${(report.duration_ms / 1000).toFixed(2)}s</p>
  <dl class="meta-grid">
    <span class="kv"><dt>started</dt><dd>${escapeHtml(meta.started_at || "n/a")}</dd></span>
    <span class="kv"><dt>platform</dt><dd>${escapeHtml(meta.platform || meta.args?.platform || "n/a")}</dd></span>
    <span class="kv"><dt>framework</dt><dd>${escapeHtml(meta.framework || meta.args?.framework || "n/a")}</dd></span>
    <span class="kv"><dt>duration</dt><dd>${(report.duration_ms / 1000).toFixed(2)}s</dd></span>
    ${argHtml}
  </dl>
  <div class="summary">
    <button type="button" class="pill" data-filter="all" aria-pressed="true">${report.total} cases</button>
    <button type="button" class="pill pass" data-filter="passed">${report.passed} passed</button>
    <button type="button" class="pill fail" data-filter="failed">${report.failed} failed</button>
    <button type="button" class="pill skip" data-filter="skipped">${report.skipped} skipped</button>
    ${report.xfail ? `<button type="button" class="pill" data-filter="xfail">${report.xfail} xfail</button>` : ""}
    ${report.xpass ? `<button type="button" class="pill fail" data-filter="xpass">${report.xpass} xpass</button>` : ""}
    ${report.timeout ? `<button type="button" class="pill fail" data-filter="timeout">${report.timeout} timeout</button>` : ""}
  </div>
  <div class="filters"><input id="q" type="search" placeholder="Filter by id, title, cover, matcher..."></div>
  ${cases}
</main>
<script>
(function () {
  var filter = "all";
  var query = "";
  var cases = Array.prototype.slice.call(document.querySelectorAll("[data-status]"));
  var pills = Array.prototype.slice.call(document.querySelectorAll("[data-filter]"));
  function apply() {
    var needle = query.toLowerCase();
    cases.forEach(function (node) {
      var status = node.getAttribute("data-status");
      var hay = (node.getAttribute("data-search") || "").toLowerCase();
      var statusOk = filter === "all" || status === filter;
      var textOk = !needle || hay.indexOf(needle) !== -1;
      node.hidden = !(statusOk && textOk);
    });
  }
  pills.forEach(function (pill) {
    pill.addEventListener("click", function () {
      filter = pill.getAttribute("data-filter") || "all";
      pills.forEach(function (other) { other.setAttribute("aria-pressed", other === pill ? "true" : "false"); });
      apply();
    });
  });
  var box = document.getElementById("q");
  if (box) box.addEventListener("input", function (event) {
    query = event.target.value || "";
    apply();
  });
})();
</script>
</body>
</html>`;
}

function flattenAssertions(item: CaseRecord): AssertionRecord[] {
  const out: AssertionRecord[] = [...(item.assertions ?? [])];
  const walk = (steps: StepRecord[]) => {
    for (const step of steps) {
      out.push(...(step.assertions ?? []));
      walk(step.steps ?? []);
    }
  };
  walk(item.steps ?? []);
  return out;
}

function renderAssertions(assertions: AssertionRecord[]): string {
  if (assertions.length === 0) return "";
  const rows = assertions
    .map((entry) => `<tr>
      <td class="${entry.passed ? "pass" : "fail"}">${entry.passed ? "pass" : "fail"}</td>
      <td>${escapeHtml(entry.matcher)}</td>
      <td>${escapeHtml(entry.expected)}</td>
      <td>${escapeHtml(entry.actual)}</td>
    </tr>`)
    .join("");
  return `<table class="assertions">
    <thead><tr><th>result</th><th>matcher</th><th>expected</th><th>actual</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>`;
}

function renderCase(item: CaseRecord): string {
  const statusClass = statusTone(item.status);
  const assertions = flattenAssertions(item);
  const passedAsserts = assertions.filter((entry) => entry.passed).length;
  const failedAsserts = assertions.length - passedAsserts;
  const covers = item.covers
    .map((tag) => `<span class="tag">${escapeHtml(tag)}</span>`)
    .join("");
  const reason = item.reason
    ? `<p class="reason">${escapeHtml(item.reason)}</p>`
    : "";
  const compare = item.error && (item.error.expected !== undefined || item.error.actual !== undefined)
    ? `<div class="compare"><div><h3>expected</h3><pre>${escapeHtml(item.error.expected ?? "")}</pre></div><div><h3>actual</h3><pre>${escapeHtml(item.error.actual ?? "")}</pre></div></div>`
    : "";
  const error = item.error
    ? `<pre>${escapeHtml(item.error.message)}</pre>${compare}`
    : "";
  const empty = assertions.length === 0 && item.steps.length === 0
    ? item.status === "skipped"
      ? `<p class="empty">Pending: no assertions (not implemented).</p>`
      : `<p class="empty">No assertions recorded.</p>`
    : "";
  const shots = item.attachments
    .filter((attachment) => attachment.mimeType.startsWith("image/") && attachment.path.endsWith(".png"))
    .map((attachment) => {
      const data = inlineFromAttachments(attachment.name, item);
      return data
        ? `<img alt="${escapeHtml(attachment.name)}" src="${data}">`
        : `<div class="attachments">${escapeHtml(attachment.path)}</div>`;
    })
    .join("");
  const files = item.attachments
    .map((attachment) => escapeHtml(attachment.path))
    .join(" | ");
  const search = [
    item.id,
    item.title,
    item.status,
    item.reason ?? "",
    ...item.covers,
    ...assertions.map((entry) => `${entry.matcher} ${entry.expected} ${entry.actual}`),
  ].join(" ");
  const assertMeta = assertions.length > 0
    ? ` &middot; ${passedAsserts} assertions passed${failedAsserts ? `, ${failedAsserts} failed` : ""}`
    : "";
  const open = item.status === "failed" || item.status === "timeout" || item.status === "xpass" ? " open" : "";
  return `<details class="case" data-status="${escapeHtml(item.status)}" data-search="${escapeHtml(search)}"${open}>
    <summary>
      <h2><span class="${statusClass}">${escapeHtml(item.status)}</span> ${escapeHtml(item.title)}</h2>
      <div class="meta">${escapeHtml(item.id)} &middot; ${(item.duration_ms / 1000).toFixed(2)}s${assertMeta}</div>
    </summary>
    <div class="body">
    ${covers ? `<div class="covers">${covers}</div>` : ""}
    ${reason}
    ${error}
    ${renderSteps(item.steps)}
    ${renderAssertions(item.assertions ?? [])}
    ${empty}
    ${shots}
    ${files ? `<div class="attachments">${files}</div>` : ""}
    </div>
  </details>`;
}

function renderSteps(steps: StepRecord[]): string {
  if (steps.length === 0) return "";
  const items = steps
    .map((step) => {
      const err = step.error ? `<pre>${escapeHtml(step.error.message)}</pre>` : "";
      return `<li class="step"><span class="${statusTone(step.status)}">${escapeHtml(step.status)}</span> ${escapeHtml(step.path)} (${step.duration_ms}ms)${err}${renderAssertions(step.assertions ?? [])}${renderSteps(step.steps)}</li>`;
    })
    .join("");
  return `<ul class="steps">${items}</ul>`;
}

function statusTone(status: SpecStatus | string): string {
  if (status === "passed" || status === "xfail") return "pass";
  if (status === "skipped") return "skip";
  return "fail";
}

const inlineStore = new Map<string, Map<string, string>>();

export function rememberInline(specId: string, name: string, dataUrl: string): void {
  let bucket = inlineStore.get(specId);
  if (!bucket) {
    bucket = new Map();
    inlineStore.set(specId, bucket);
  }
  bucket.set(name, dataUrl);
}

export function clearInline(): void {
  inlineStore.clear();
}

function inlineFromAttachments(name: string, item: CaseRecord): string | undefined {
  return inlineStore.get(item.id)?.get(name);
}

export function dataUrl(mimeType: string, base64: string): string {
  return `data:${mimeType};base64,${base64}`;
}

export function countStatuses(cases: CaseRecord[]): Omit<JsonReport, "framework" | "meta" | "partial" | "filtered" | "duration_ms" | "cases"> {
  const counts = {
    total: cases.length,
    passed: 0,
    failed: 0,
    skipped: 0,
    xfail: 0,
    xpass: 0,
    timeout: 0,
  };
  for (const item of cases) {
    if (item.status === "passed") counts.passed += 1;
    else if (item.status === "failed") counts.failed += 1;
    else if (item.status === "skipped") counts.skipped += 1;
    else if (item.status === "xfail") counts.xfail += 1;
    else if (item.status === "xpass") counts.xpass += 1;
    else if (item.status === "timeout") counts.timeout += 1;
  }
  return counts;
}
