import { escapeHtml } from "./format.js";
import type { CaseRecord, JsonReport, SpecStatus, StepRecord } from "./types.js";
import { PACKAGE_NAME, VERSION } from "./version.js";

export function renderHtml(report: JsonReport): string {
  const cases = report.cases.map(renderCase).join("");
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
main { max-width: 960px; margin: 0 auto; padding: 32px 20px 64px; }
h1 { font-size: 22px; margin: 0 0 8px; }
.meta { color: var(--muted); margin-bottom: 24px; }
.summary { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 28px; }
.pill { padding: 4px 10px; border-radius: 999px; background: var(--card); border: 1px solid var(--line); }
.pass { color: var(--pass); }
.fail { color: var(--fail); }
.skip { color: var(--skip); }
.case { background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 16px 18px; margin-bottom: 14px; }
.case h2 { font-size: 16px; margin: 0 0 6px; }
.covers { display: flex; flex-wrap: wrap; gap: 6px; margin: 8px 0; }
.tag { font-size: 12px; padding: 2px 8px; border-radius: 999px; background: var(--bg); color: var(--muted); }
.steps { margin: 10px 0 0; padding: 0; list-style: none; }
.steps ul { margin: 6px 0 0 16px; padding: 0; list-style: none; }
.step { margin: 4px 0; }
pre { white-space: pre-wrap; background: var(--bg); padding: 10px 12px; border-radius: 8px; overflow: auto; }
img { max-width: 100%; border-radius: 8px; margin-top: 8px; border: 1px solid var(--line); }
.attachments { margin-top: 8px; color: var(--muted); font-size: 12px; }
</style>
</head>
<body>
<main>
  <h1>lxdev test report</h1>
  <p class="meta">${escapeHtml(PACKAGE_NAME)} ${escapeHtml(VERSION)}${report.partial ? " · partial" : ""}${report.filtered ? " · filtered" : ""} · ${(report.duration_ms / 1000).toFixed(2)}s</p>
  <div class="summary">
    <span class="pill">${report.total} cases</span>
    <span class="pill pass">${report.passed} passed</span>
    <span class="pill fail">${report.failed} failed</span>
    <span class="pill skip">${report.skipped} skipped</span>
    ${report.xfail ? `<span class="pill">${report.xfail} xfail</span>` : ""}
    ${report.xpass ? `<span class="pill fail">${report.xpass} xpass</span>` : ""}
    ${report.timeout ? `<span class="pill fail">${report.timeout} timeout</span>` : ""}
  </div>
  ${cases}
</main>
</body>
</html>`;
}

function renderCase(item: CaseRecord): string {
  const statusClass = statusTone(item.status);
  const covers = item.covers
    .map((tag) => `<span class="tag">${escapeHtml(tag)}</span>`)
    .join("");
  const error = item.error
    ? `<pre>${escapeHtml(item.error.message)}</pre>`
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
    .join(" · ");
  return `<article class="case">
    <h2><span class="${statusClass}">${escapeHtml(item.status)}</span> ${escapeHtml(item.title)}</h2>
    <div class="meta">${escapeHtml(item.id)} · ${(item.duration_ms / 1000).toFixed(2)}s</div>
    ${covers ? `<div class="covers">${covers}</div>` : ""}
    ${error}
    ${renderSteps(item.steps)}
    ${shots}
    ${files ? `<div class="attachments">${files}</div>` : ""}
  </article>`;
}

function renderSteps(steps: StepRecord[]): string {
  if (steps.length === 0) return "";
  const items = steps
    .map((step) => {
      const err = step.error ? `<pre>${escapeHtml(step.error.message)}</pre>` : "";
      return `<li class="step"><span class="${statusTone(step.status)}">${escapeHtml(step.status)}</span> ${escapeHtml(step.path)} (${step.duration_ms}ms)${err}${renderSteps(step.steps)}</li>`;
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

export function countStatuses(cases: CaseRecord[]): Omit<JsonReport, "framework" | "partial" | "filtered" | "duration_ms" | "cases"> {
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
