/**
 * Report panels for tags, the `--covers-manifest` summary and the
 * `--openapi` contract, kept apart from the core report renderer.
 */
import { escapeHtml } from "./format.js";
import type { CaseRecord, ContractIssue, CoverageSummary, JsonReport, OpenApiSummary, TagSummary } from "./types.js";

function toneOf(status: string): string {
  if (status === "passed" || status === "xfail") return "pass";
  if (status === "skipped" || status === "not_run") return "skip";
  if (status === "uncovered") return "none";
  return "fail";
}

/** One row per tag, so a failing `live` group cannot hide a clean `routed` one. */
export function renderTagSummary(rows: readonly TagSummary[] | undefined): string {
  if (!rows || rows.length === 0) return "";
  const failing = rows.filter((row) => !row.ok).length;
  const body = rows.map((row) => {
    const counts = (["passed", "failed", "timeout", "xpass", "xfail", "skipped"] as const)
      .filter((status) => row[status] > 0)
      .map((status) => `<span class="mini ${toneOf(status)}">${row[status]} ${status}</span>`)
      .join("");
    const flaky = row.flaky > 0 ? `<span class="mini skip">${row.flaky} flaky</span>` : "";
    return `<tr class="${row.ok ? "row-ok" : "row-fail"}">
      <td><span class="tag">${escapeHtml(row.tag)}</span></td>
      <td class="num">${row.total}</td>
      <td>${counts}${flaky}</td>
    </tr>`;
  }).join("");
  return `<details class="panel" open>
    <summary><span class="panel-title">By tag</span>
      <span class="panel-sub">${rows.length} tag${rows.length === 1 ? "" : "s"}${failing > 0 ? `, ${failing} failing` : ", all passing"}</span></summary>
    <div class="panel-body"><table class="tagsum"><thead><tr><th>tag</th><th>specs</th><th>outcome</th></tr></thead><tbody>${body}</tbody></table></div>
  </details>`;
}

/** Manifest ids: covered by which specs and how they fared, and the holes. */
export function renderManifestCoverage(coverage: CoverageSummary | undefined): string {
  if (!coverage) return "";
  const rows = coverage.ids.map((entry) => {
    const specs = entry.specs.length === 0
      ? `<span class="muted">no spec</span>`
      : entry.specs.map((spec) =>
        `<a class="mini ${toneOf(spec.status)}" href="#case-${escapeHtml(spec.id)}" title="${escapeHtml(spec.title)}">${escapeHtml(spec.id)} · ${escapeHtml(spec.status.replace("_", " "))}</a>`).join(" ");
    return `<tr data-search="${escapeHtml(`${entry.id} ${entry.title ?? ""}`)}">
      <td><span class="cover cover-${entry.status === "passed" ? "ok" : entry.status === "uncovered" ? "none" : entry.status === "failed" ? "claimed" : "pending"}">${escapeHtml(entry.id)}</span></td>
      <td>${escapeHtml(entry.title ?? "")}</td>
      <td>${specs}</td>
    </tr>`;
  }).join("");
  const unknown = coverage.unknown.length > 0
    ? `<p class="warn-line">Not in the manifest: ${coverage.unknown.map((entry) =>
      `<code>${escapeHtml(entry.id)}</code> (${entry.specs.map(escapeHtml).join(", ")})`).join(", ")}</p>`
    : "";
  return `<details class="panel" open>
    <summary><span class="panel-title">Coverage manifest</span>
      <span class="panel-sub">${coverage.covered}/${coverage.total} ids covered · ${coverage.passing} passing · ${coverage.failing} failing · ${coverage.uncovered.length} without a spec</span></summary>
    <div class="panel-body">
      ${unknown}
      <table class="tagsum"><thead><tr><th>id</th><th>title</th><th>specs</th></tr></thead><tbody>${rows}</tbody></table>
    </div>
  </details>`;
}

/** The `--openapi` run summary. */
export function renderContract(openapi: OpenApiSummary | undefined): string {
  if (!openapi) return "";
  const docs = openapi.documents.map((doc) =>
    `<span class="chip"><b>${escapeHtml(doc.name)}</b>${escapeHtml(`${doc.title ? `${doc.title} · ` : ""}OpenAPI ${doc.version} · ${doc.operations} operations`)}</span>`).join("");
  const skipped = Object.entries(openapi.skipped).filter(([, count]) => count > 0)
    .map(([reason, count]) => `${count} ${reason.replace("_", " ")}`).join(", ");
  const list = (title: string, items: string[]) => items.length === 0 ? "" : `<h4>${escapeHtml(title)}</h4><ul class="plain">${items.join("")}</ul>`;
  const undocumented = list("Statuses the contract does not document",
    openapi.undocumented.map((entry) => `<li><code>${escapeHtml(entry.operation)} → ${entry.status}</code> <span class="muted">${escapeHtml(entry.source)} ×${entry.count}</span></li>`));
  const unmatched = list("Requests no operation describes",
    openapi.unmatched.map((entry) => `<li><code>${escapeHtml(`${entry.method} ${entry.path}`)}</code> <span class="muted">×${entry.count}</span></li>`));
  const warnings = list("Server responses that break the contract (warnings)",
    openapi.warnings.map((issue) => `<li>${renderIssue(issue)}</li>`));
  const failed = openapi.routed.failed > 0;
  return `<details class="panel"${failed || openapi.warnings.length > 0 ? " open" : ""}>
    <summary><span class="panel-title">OpenAPI contract</span>
      <span class="panel-sub">${openapi.validated}/${openapi.responses} responses validated · routed ${openapi.routed.validated - openapi.routed.failed}/${openapi.routed.validated} ok · server ${openapi.network.validated - openapi.network.mismatched}/${openapi.network.validated} ok${skipped ? ` · skipped: ${escapeHtml(skipped)}` : ""}</span></summary>
    <div class="panel-body">
      <div class="chips">${docs}</div>
      ${warnings}${undocumented}${unmatched}
    </div>
  </details>`;
}

function renderIssue(issue: ContractIssue): string {
  const lines = issue.issues.map((entry) => `at ${entry.path || "/"}: ${entry.message} (${entry.schema})`).join("\n");
  return `<a href="#case-${escapeHtml(issue.case)}"><code>${escapeHtml(issue.case)}</code></a>
    <code>${escapeHtml(`${issue.operation} → ${issue.status}`)}</code> against <code>${escapeHtml(issue.schema)}</code>
    <pre class="message">${escapeHtml(lines)}</pre>`;
}

/** Tag chips for a case. */
export function renderCaseTags(item: CaseRecord): string {
  const tags = item.tags ?? [];
  return tags.length === 0 ? "" : `<div class="covers">${tags.map((tag) => `<span class="tag tag-sel">#${escapeHtml(tag)}</span>`).join("")}</div>`;
}

/** Contract warnings for a case (violations already are its error). */
export function renderCaseContract(item: CaseRecord): string {
  const contract = item.contract;
  if (!contract || contract.warnings.length === 0) return "";
  return `<div class="contract"><h4>OpenAPI warnings (server responses)</h4>${contract.warnings.map(renderIssue).join("")}</div>`;
}

/** Extra search terms for a case. */
export function caseSearchTerms(item: CaseRecord): string[] {
  return (item.tags ?? []).flatMap((tag) => [tag, `#${tag}`]);
}

export function hasExtras(report: JsonReport): boolean {
  return Boolean(report.tag_summary?.length || report.coverage || report.openapi);
}

export const EXTRA_STYLE = `
table.tagsum { width:100%; border-collapse:collapse; font-size:13px; }
table.tagsum th, table.tagsum td { text-align:left; padding:5px 8px; border-bottom:1px solid var(--line); vertical-align:top; }
table.tagsum th { color:var(--muted); font-size:11px; text-transform:uppercase; letter-spacing:.05em; }
table.tagsum td.num { text-align:right; font-family:var(--mono); width:4em; }
table.tagsum td .mini { margin-right:4px; display:inline-block; text-decoration:none; }
tr.row-fail td:first-child { border-left:3px solid var(--fail); }
tr.row-ok td:first-child { border-left:3px solid var(--pass); }
.tag-sel { color:var(--accent); }
.muted { color:var(--muted); }
.warn-line { color:var(--skip); margin:0 0 10px; }
ul.plain { margin:4px 0 12px; padding-left:18px; }
.contract { margin:8px 0; }
.contract h4, .panel-body h4 { margin:10px 0 4px; font-size:12px; color:var(--muted); text-transform:uppercase; letter-spacing:.05em; }
`;
