import { escapeHtml } from "./format.js";
import type {
  AssertionRecord,
  CaseRecord,
  JsonReport,
  SpecStatus,
  StepRecord,
} from "./types.js";
import {
  CAPABILITY_INDEX,
  LAYER_TITLE,
  PUBLIC_CAPABILITIES,
  type CapabilityLayer,
} from "./inventory.js";
import { PACKAGE_NAME, VERSION } from "./version.js";

/** A status the report groups, filters, and colours by. */
const STATUS_ORDER: SpecStatus[] = [
  "failed",
  "timeout",
  "xpass",
  "passed",
  "xfail",
  "skipped",
];

const STATUS_LABEL: Record<SpecStatus, string> = {
  passed: "passed",
  failed: "failed",
  skipped: "pending",
  timeout: "timed out",
  xfail: "expected fail",
  xpass: "unexpected pass",
};

interface Suite {
  name: string;
  cases: CaseRecord[];
}

/** What a run proved about one capability. */
interface CoverState {
  /** A passing spec asserted the capability's behaviour. */
  behaviour: boolean;
  /** A passing spec only proved the member exists (`shape:` tag). */
  shape: boolean;
  /** Every spec that declared the tag, passing or not. */
  specs: string[];
  /** Declared somewhere, but only by a pending or failing spec. */
  declared: boolean;
}

interface CoverGroup {
  group: string;
  layer: CapabilityLayer | "custom";
  rows: Array<{ name: string; state: CoverState }>;
  behaviour: number;
  shape: number;
}

export function renderHtml(report: JsonReport): string {
  const meta = report.meta ?? { started_at: "", duration_ms: report.duration_ms, args: {} };
  const args = meta.args ?? {};
  const suites = groupSuites(report.cases);
  const stats = collectStats(report);
  const verdict = verdictOf(report);

  const subject = meta.subject;
  // The report is about the app, not about the tool that ran it.
  const appName = subject?.app_name || subject?.appid || "";
  const documentTitle = appName ? `${appName} test report` : "lxapp test report";
  const chips = [
    ...(subject?.appid && subject.appid !== appName ? [chip("id", subject.appid)] : []),
    ...(subject?.version ? [chip("version", subject.version)] : []),
    ...(subject?.release_type ? [chip("build", subject.release_type)] : []),
    chip("platform", meta.platform || args.platform || "—"),
    chip("framework", meta.framework || args.framework || "—"),
    chip("started", meta.started_at ? meta.started_at.replace("T", " ").replace(/\.\d+Z$/, "Z") : "—"),
    chip("duration", formatDuration(report.duration_ms)),
    chip("runner", `${PACKAGE_NAME} ${VERSION}`),
    ...Object.entries(args)
      .filter(([key]) => key !== "platform" && key !== "framework")
      .map(([key, value]) => chip(key, value)),
  ].join("");

  const flags = [
    report.partial ? flag("partial", "The run ended early; results below are incomplete.") : "",
    report.filtered ? flag("filtered", "A --grep or spec.only selection was in effect.") : "",
  ].filter(Boolean).join("");

  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(documentTitle)}</title>
<style>${STYLE}</style>
</head>
<body>
<a class="skip-link" href="#results">Skip to results</a>
<main>
  <header class="hero ${verdict.tone}">
    <div class="hero-text">
      <p class="eyebrow">${escapeHtml(appName || "lxapp")} &middot; test report</p>
      <h1><span class="verdict">${escapeHtml(verdict.label)}</span></h1>
      <p class="lede">${escapeHtml(verdict.detail)}</p>
      <div class="chips">${chips}</div>
      ${flags ? `<div class="flags">${flags}</div>` : ""}
    </div>
    ${renderDonut(report)}
  </header>

  <section class="metrics" aria-label="Run totals">
    ${metric("specs", String(report.total), "registered and selected")}
    ${metric("assertions", String(stats.assertions), `${stats.failedAssertions} failed`)}
    ${metric("steps", String(stats.steps), "recorded")}
    ${metric("lx API", percent(stats.logicBehaviour, stats.logicTotal), `${stats.logicBehaviour}/${stats.logicTotal} Logic capabilities behaviour-proven`)}
  </section>

  ${renderBar(report)}

  <div class="toolbar" role="search">
    <input id="q" type="search" placeholder="Search id, title, suite, cover tag, matcher…" aria-label="Filter specs">
    <div class="pills" role="group" aria-label="Filter by status">
      ${renderPill("all", `all ${report.total}`, true, "")}
      ${STATUS_ORDER.filter((status) => countOf(report, status) > 0)
        .map((status) => renderPill(status, `${STATUS_LABEL[status]} ${countOf(report, status)}`, false, tone(status)))
        .join("")}
    </div>
    <div class="actions">
      <button type="button" id="expand">Expand all</button>
      <button type="button" id="collapse">Collapse all</button>
      <button type="button" id="theme" aria-label="Toggle colour theme">Theme</button>
    </div>
  </div>

  ${renderCoverage(report)}
  ${renderSlowest(report)}

  <div id="results">
    ${suites.map((suite) => renderSuite(suite)).join("")}
  </div>
  <p class="no-match" id="no-match" hidden>Nothing matches this filter.</p>

  <footer>
    Generated by ${escapeHtml(PACKAGE_NAME)} ${escapeHtml(VERSION)} &middot; single file, no network access.
  </footer>
</main>
<script>${SCRIPT}</script>
</body>
</html>`;
}

/* ------------------------------------------------------------------ */
/* structure                                                           */
/* ------------------------------------------------------------------ */

function groupSuites(cases: CaseRecord[]): Suite[] {
  const suites: Suite[] = [];
  const index = new Map<string, Suite>();
  for (const item of cases) {
    const name = item.suite ?? "specs";
    let suite = index.get(name);
    if (!suite) {
      suite = { name, cases: [] };
      index.set(name, suite);
      suites.push(suite);
    }
    suite.cases.push(item);
  }
  return suites;
}

function countOf(report: JsonReport, status: SpecStatus): number {
  switch (status) {
    case "passed": return report.passed;
    case "failed": return report.failed;
    case "skipped": return report.skipped;
    case "timeout": return report.timeout;
    case "xfail": return report.xfail;
    case "xpass": return report.xpass;
    default: return 0;
  }
}

function collectStats(report: JsonReport): {
  assertions: number;
  failedAssertions: number;
  steps: number;
  logicBehaviour: number;
  logicTotal: number;
} {
  let assertions = 0;
  let failedAssertions = 0;
  let steps = 0;
  for (const item of report.cases) {
    const flat = flattenAssertions(item);
    assertions += flat.length;
    failedAssertions += flat.filter((entry) => !entry.passed).length;
    steps += countSteps(item.steps);
  }
  const logic = coverageTotals(collectCovers(report), "logic");
  return {
    assertions,
    failedAssertions,
    steps,
    logicBehaviour: logic.behaviour,
    logicTotal: logic.total,
  };
}

function countSteps(steps: StepRecord[]): number {
  let total = 0;
  for (const step of steps) total += 1 + countSteps(step.steps ?? []);
  return total;
}

function verdictOf(report: JsonReport): { label: string; detail: string; tone: string } {
  const broken = report.failed + report.timeout + report.xpass;
  if (report.partial) {
    return {
      label: "Incomplete",
      detail: `The run stopped before every spec reported. ${report.passed} of ${report.total} specs passed so far.`,
      tone: "tone-warn",
    };
  }
  if (broken > 0) {
    return {
      label: "Failed",
      detail: broken === 1
        ? "1 spec did not hold its contract."
        : `${broken} specs did not hold their contract.`,
      tone: "tone-fail",
    };
  }
  if (report.total === 0) {
    return { label: "Empty", detail: "No spec matched this selection.", tone: "tone-warn" };
  }
  return {
    label: "Passed",
    detail: report.skipped > 0
      ? `${report.passed} specs held their contract; ${report.skipped} stay pending.`
      : `All ${report.passed} specs held their contract.`,
    tone: "tone-pass",
  };
}

/* ------------------------------------------------------------------ */
/* header widgets                                                      */
/* ------------------------------------------------------------------ */

function chip(key: string, value: string): string {
  return `<span class="chip"><b>${escapeHtml(key)}</b>${escapeHtml(value)}</span>`;
}

function flag(name: string, hint: string): string {
  return `<span class="flag" title="${escapeHtml(hint)}">${escapeHtml(name)}</span>`;
}

function metric(label: string, value: string, hint: string): string {
  return `<div class="metric"><dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd><small>${escapeHtml(hint)}</small></div>`;
}

function renderDonut(report: JsonReport): string {
  const graded = report.passed + report.xfail + report.failed + report.timeout + report.xpass;
  const rate = graded > 0 ? (report.passed + report.xfail) / graded : 0;
  const circumference = 2 * Math.PI * 52;
  const dash = (rate * circumference).toFixed(2);
  const percent = graded > 0 ? `${Math.round(rate * 100)}%` : "—";
  return `<div class="donut" role="img" aria-label="${escapeHtml(percent)} of graded specs passed">
    <svg viewBox="0 0 120 120" width="132" height="132">
      <circle class="donut-track" cx="60" cy="60" r="52"></circle>
      <circle class="donut-value" cx="60" cy="60" r="52"
        stroke-dasharray="${dash} ${(circumference - Number(dash)).toFixed(2)}"
        transform="rotate(-90 60 60)"></circle>
    </svg>
    <div class="donut-label"><strong>${escapeHtml(percent)}</strong><span>graded pass rate</span></div>
  </div>`;
}

function renderBar(report: JsonReport): string {
  const segments = STATUS_ORDER
    .map((status) => ({ status, count: countOf(report, status) }))
    .filter((entry) => entry.count > 0);
  if (segments.length === 0) return "";
  const total = segments.reduce((sum, entry) => sum + entry.count, 0);
  const parts = segments
    .map((entry) => `<span class="seg ${tone(entry.status)}" style="flex:${entry.count}"
      title="${escapeHtml(`${entry.count} ${STATUS_LABEL[entry.status]}`)}"></span>`)
    .join("");
  return `<div class="bar" role="img" aria-label="${escapeHtml(
    segments.map((entry) => `${entry.count} ${STATUS_LABEL[entry.status]}`).join(", "),
  )}">${parts}</div><p class="bar-caption">${total} spec${total === 1 ? "" : "s"} &middot; ${formatDuration(report.duration_ms)}</p>`;
}

function renderPill(filter: string, label: string, pressed: boolean, toneClass: string): string {
  return `<button type="button" class="pill ${toneClass}" data-filter="${escapeHtml(filter)}" aria-pressed="${pressed}">${escapeHtml(label)}</button>`;
}

/* ------------------------------------------------------------------ */
/* coverage                                                            */
/* ------------------------------------------------------------------ */

function emptyState(): CoverState {
  return { behaviour: false, shape: false, specs: [], declared: false };
}

function collectCovers(report: JsonReport): Map<string, CoverState> {
  const states = new Map<string, CoverState>();
  const touch = (name: string): CoverState => {
    let state = states.get(name);
    if (!state) {
      state = emptyState();
      states.set(name, state);
    }
    return state;
  };
  for (const capability of PUBLIC_CAPABILITIES) touch(capability.name);
  for (const item of report.cases) {
    const proven = item.status === "passed" || item.status === "xfail";
    for (const tag of item.covers) {
      const shapeOnly = tag.startsWith("shape:");
      const name = shapeOnly ? tag.slice("shape:".length) : tag;
      const state = touch(name);
      state.declared = true;
      if (!state.specs.includes(item.id)) state.specs.push(item.id);
      if (!proven) continue;
      if (shapeOnly) state.shape = true;
      else state.behaviour = true;
    }
  }
  return states;
}

function groupCovers(states: Map<string, CoverState>): CoverGroup[] {
  const groups = new Map<string, CoverGroup>();
  const bucket = (group: string, layer: CapabilityLayer | "custom"): CoverGroup => {
    let entry = groups.get(group);
    if (!entry) {
      entry = { group, layer, rows: [], behaviour: 0, shape: 0 };
      groups.set(group, entry);
    }
    return entry;
  };
  for (const [name, state] of states) {
    const capability = CAPABILITY_INDEX.get(name);
    // A tag outside the published surface is still worth showing — it is
    // either a project-local contract or a stale tag after an API rename.
    const entry = capability
      ? bucket(capability.group, capability.layer)
      : bucket("project tags", "custom");
    entry.rows.push({ name, state });
    if (state.behaviour) entry.behaviour += 1;
    if (state.shape || state.behaviour) entry.shape += 1;
  }
  for (const entry of groups.values()) {
    entry.rows.sort((left, right) => left.name.localeCompare(right.name));
  }
  return [...groups.values()].sort((left, right) => left.group.localeCompare(right.group));
}

function coverageTotals(states: Map<string, CoverState>, layer: CapabilityLayer): {
  behaviour: number;
  shape: number;
  total: number;
} {
  let behaviour = 0;
  let shape = 0;
  let total = 0;
  for (const capability of PUBLIC_CAPABILITIES) {
    if (capability.layer !== layer) continue;
    total += 1;
    const state = states.get(capability.name);
    if (!state) continue;
    if (state.behaviour) behaviour += 1;
    if (state.shape || state.behaviour) shape += 1;
  }
  return { behaviour, shape, total };
}

function percent(part: number, whole: number): string {
  return whole > 0 ? `${Math.round((part / whole) * 100)}%` : "—";
}

function renderCoverage(report: JsonReport): string {
  const states = collectCovers(report);
  const groups = groupCovers(states);
  const sections = (["logic", "object", "automation", "custom"] as const)
    .map((layer) => renderCoverageSection(layer, groups.filter((group) => group.layer === layer), states))
    .filter(Boolean)
    .join("");
  if (sections.length === 0) return "";
  const logic = coverageTotals(states, "logic");
  return `<details class="panel" open>
    <summary><span class="panel-title">lx API coverage</span>
      <span class="panel-sub">${logic.behaviour}/${logic.total} Logic capabilities behaviour-proven
      (${percent(logic.behaviour, logic.total)}), ${percent(logic.shape, logic.total)} shape-proven</span></summary>
    <div class="panel-body">
      <ul class="legend">
        <li><span class="cover cover-ok">behaviour</span> a passing spec asserted it</li>
        <li><span class="cover cover-shape">shape</span> proven only to exist</li>
        <li><span class="cover cover-pending">declared</span> claimed by a pending or failing spec</li>
        <li><span class="cover cover-none">uncovered</span> no spec at all</li>
      </ul>
      ${sections}
    </div>
  </details>`;
}

function renderCoverageSection(
  layer: CapabilityLayer | "custom",
  groups: CoverGroup[],
  states: Map<string, CoverState>,
): string {
  if (groups.length === 0) return "";
  const title = layer === "custom" ? "Project tags" : LAYER_TITLE[layer];
  const head = layer === "custom"
    ? `${groups.reduce((sum, group) => sum + group.rows.length, 0)} tags outside the published surface`
    : (() => {
        const totals = coverageTotals(states, layer);
        return `${totals.behaviour}/${totals.total} behaviour &middot; ${totals.shape}/${totals.total} shape`;
      })();
  const body = groups.map((group) => {
    const chips = group.rows.map(({ name, state }) => {
      const klass = state.behaviour
        ? "cover-ok"
        : state.shape
          ? "cover-shape"
          : state.declared
            ? "cover-pending"
            : "cover-none";
      const hint = state.specs.length > 0 ? `covered by ${state.specs.join(", ")}` : "no spec declares this tag";
      return `<span class="cover ${klass}" title="${escapeHtml(hint)}"
        data-search="${escapeHtml(`${name} ${state.specs.join(" ")}`)}">${escapeHtml(name)}</span>`;
    }).join("");
    return `<div class="cover-group">
      <h3>${escapeHtml(group.group)} <small>${group.behaviour}/${group.rows.length}</small></h3>
      <div class="cover-tags">${chips}</div>
    </div>`;
  }).join("");
  return `<section class="cover-section">
    <h2 class="cover-section-head">${escapeHtml(title)} <small>${head}</small></h2>
    <div class="cover-grid">${body}</div>
  </section>`;
}

function renderSlowest(report: JsonReport): string {
  const ranked = report.cases
    .filter((item) => item.status !== "skipped")
    .slice()
    .sort((a, b) => b.duration_ms - a.duration_ms)
    .slice(0, 8);
  if (ranked.length < 2) return "";
  const max = ranked[0]!.duration_ms || 1;
  const rows = ranked.map((item) => `<tr>
      <td><a href="#case-${escapeHtml(item.id)}">${escapeHtml(item.title)}</a></td>
      <td class="num">${formatDuration(item.duration_ms)}</td>
      <td class="spark"><span style="width:${Math.max(2, Math.round((item.duration_ms / max) * 100))}%"></span></td>
    </tr>`).join("");
  return `<details class="panel">
    <summary><span class="panel-title">Slowest specs</span>
      <span class="panel-sub">where the wall clock went</span></summary>
    <div class="panel-body"><table class="slowest"><tbody>${rows}</tbody></table></div>
  </details>`;
}

/* ------------------------------------------------------------------ */
/* suites and cases                                                    */
/* ------------------------------------------------------------------ */

function renderSuite(suite: Suite): string {
  const counts = STATUS_ORDER
    .map((status) => ({ status, count: suite.cases.filter((item) => item.status === status).length }))
    .filter((entry) => entry.count > 0)
    .map((entry) => `<span class="mini ${tone(entry.status)}">${entry.count} ${escapeHtml(STATUS_LABEL[entry.status])}</span>`)
    .join("");
  const duration = suite.cases.reduce((sum, item) => sum + item.duration_ms, 0);
  return `<section class="suite" data-suite="${escapeHtml(suite.name)}">
    <h2 class="suite-head">
      <span class="suite-name">${escapeHtml(suite.name)}</span>
      <span class="suite-meta">${counts}<span class="mini">${formatDuration(duration)}</span></span>
    </h2>
    ${suite.cases.map((item) => renderCase(item, suite.name)).join("")}
  </section>`;
}

function renderCase(item: CaseRecord, suiteName: string): string {
  const assertions = flattenAssertions(item);
  const passedAsserts = assertions.filter((entry) => entry.passed).length;
  const failedAsserts = assertions.length - passedAsserts;
  const covers = item.covers.length > 0
    ? `<div class="covers">${item.covers.map((tag) => `<span class="tag">${escapeHtml(tag)}</span>`).join("")}</div>`
    : "";
  const reason = item.reason ? `<p class="reason">${escapeHtml(item.reason)}</p>` : "";
  const error = renderError(item);
  const empty = assertions.length === 0 && item.steps.length === 0
    ? item.status === "skipped"
      ? `<p class="empty">Pending: registered as a known hole, no assertions run.</p>`
      : `<p class="empty">No assertion was recorded for this spec.</p>`
    : "";
  const shots = renderAttachments(item);
  const files = "";
  const search = [
    item.id,
    item.title,
    item.status,
    suiteName,
    item.reason ?? "",
    item.error?.message ?? "",
    ...item.covers,
    ...assertions.map((entry) => `${entry.matcher} ${entry.expected} ${entry.actual}`),
  ].join(" ");
  const assertMeta = assertions.length > 0
    ? ` &middot; ${passedAsserts} assertion${passedAsserts === 1 ? "" : "s"} passed${failedAsserts ? `, ${failedAsserts} failed` : ""}`
    : "";
  const open = isBroken(item.status) ? " open" : "";
  const where = item.file ? ` &middot; ${escapeHtml(item.file)}${item.line ? `:${item.line}` : ""}` : "";
  return `<details class="case ${tone(item.status)}" id="case-${escapeHtml(item.id)}"
    data-status="${escapeHtml(item.status)}" data-search="${escapeHtml(search)}"${open}>
    <summary>
      <span class="badge ${tone(item.status)}">${escapeHtml(STATUS_LABEL[item.status] ?? item.status)}</span>
      <span class="case-title">${escapeHtml(item.title)}</span>
      <span class="case-time">${formatDuration(item.duration_ms)}</span>
    </summary>
    <div class="body">
      <p class="meta"><code>${escapeHtml(item.id)}</code>${where}${assertMeta}</p>
      ${covers}
      ${reason}
      ${error}
      ${renderSteps(item.steps)}
      ${renderAssertions(item.assertions ?? [], "spec assertions")}
      ${empty}
      ${shots}
      ${files}
    </div>
  </details>`;
}

/**
 * An artifact the reader cannot open from the report is an artifact they will
 * not look at, so images inline and text/JSON preview in place. Everything
 * else names the path the CLI wrote it to.
 */
function renderAttachments(item: CaseRecord): string {
  if (item.attachments.length === 0) return "";
  const blocks = item.attachments.map((attachment) => {
    const inlined = inlineFromAttachments(attachment.name, item);
    // The path is relative to the report, so the link opens the real file
    // whenever the report is read from the directory the CLI wrote it to.
    const link = `<a class="path" href="${escapeHtml(attachment.path)}">${escapeHtml(attachment.path)}</a>`;
    if (inlined?.dataUrl) {
      return `<figure class="shot">
        <img alt="${escapeHtml(attachment.name)}" src="${inlined.dataUrl}" loading="lazy">
        <figcaption>${escapeHtml(attachment.name)} &middot; ${link}</figcaption>
      </figure>`;
    }
    if (inlined?.text !== undefined) {
      return `<details class="artifact">
        <summary>${escapeHtml(attachment.name)} ${link}</summary>
        <pre>${escapeHtml(inlined.text)}</pre>
      </details>`;
    }
    return `<div class="artifact-path"><b>${escapeHtml(attachment.name)}</b> ${link}</div>`;
  });
  return `<div class="artifacts"><h4>artifacts</h4>${blocks.join("")}</div>`;
}

function renderError(item: CaseRecord): string {
  if (!item.error) return "";
  const error = item.error;
  const compare = error.expected !== undefined || error.actual !== undefined
    ? `<div class="compare">
        <div class="side expected"><h4>expected</h4><pre>${escapeHtml(error.expected ?? "—")}</pre></div>
        <div class="side actual"><h4>actual</h4><pre>${escapeHtml(error.actual ?? "—")}</pre></div>
      </div>`
    : "";
  const at = error.location ? `<p class="at">at ${escapeHtml(error.location)}</p>` : "";
  const inStep = error.step ? `<p class="at">in step <code>${escapeHtml(error.step)}</code></p>` : "";
  const stack = error.stack
    ? `<details class="stack"><summary>stack</summary><pre>${escapeHtml(error.stack)}</pre></details>`
    : "";
  return `<div class="failure">
    <h3>${escapeHtml(error.name)}${error.matcher ? ` &middot; <code>${escapeHtml(error.matcher)}</code>` : ""}</h3>
    <pre class="message">${escapeHtml(error.message)}</pre>
    ${compare}${at}${inStep}${stack}
  </div>`;
}

function renderSteps(steps: StepRecord[]): string {
  if (steps.length === 0) return "";
  const total = Math.max(1, ...steps.map((step) => step.duration_ms));
  const items = steps.map((step) => {
    const err = step.error ? `<pre class="message">${escapeHtml(step.error.message)}</pre>` : "";
    const width = Math.max(2, Math.round((step.duration_ms / total) * 100));
    return `<li class="step">
      <div class="step-head">
        <span class="badge sm ${tone(step.status)}">${escapeHtml(step.status)}</span>
        <span class="step-name">${escapeHtml(step.name)}</span>
        <span class="step-time">${step.duration_ms}ms</span>
        <span class="step-bar"><span style="width:${width}%"></span></span>
      </div>
      ${err}${renderAssertions(step.assertions ?? [], "")}${renderSteps(step.steps ?? [])}
    </li>`;
  }).join("");
  return `<ul class="steps">${items}</ul>`;
}

function renderAssertions(assertions: AssertionRecord[], caption: string): string {
  if (assertions.length === 0) return "";
  const rows = assertions.map((entry) => `<tr class="${entry.passed ? "row-pass" : "row-fail"}">
      <td class="${entry.passed ? "pass" : "fail"}">${entry.passed ? "pass" : "fail"}</td>
      <td><code>${escapeHtml(entry.matcher)}</code></td>
      <td>${escapeHtml(entry.expected)}</td>
      <td>${escapeHtml(entry.actual)}</td>
    </tr>`).join("");
  return `<table class="assertions">
    ${caption ? `<caption>${escapeHtml(caption)}</caption>` : ""}
    <thead><tr><th>result</th><th>matcher</th><th>expected</th><th>actual</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>`;
}

/* ------------------------------------------------------------------ */
/* helpers                                                             */
/* ------------------------------------------------------------------ */

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

function isBroken(status: SpecStatus): boolean {
  return status === "failed" || status === "timeout" || status === "xpass";
}

function tone(status: SpecStatus | string): string {
  if (status === "passed" || status === "xfail") return "pass";
  if (status === "skipped") return "skip";
  return "fail";
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)}s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = ((ms % 60_000) / 1000).toFixed(0).padStart(2, "0");
  return `${minutes}m ${seconds}s`;
}

interface Inlined {
  /** `data:` URL for an image. */
  dataUrl?: string;
  /** Decoded text for a text/JSON attachment. */
  text?: string;
}

const inlineStore = new Map<string, Map<string, Inlined>>();

export function rememberInline(specId: string, name: string, payload: Inlined): void {
  let bucket = inlineStore.get(specId);
  if (!bucket) {
    bucket = new Map();
    inlineStore.set(specId, bucket);
  }
  bucket.set(name, payload);
}

export function clearInline(): void {
  inlineStore.clear();
}

function inlineFromAttachments(name: string, item: CaseRecord): Inlined | undefined {
  return inlineStore.get(item.id)?.get(name);
}

export function dataUrl(mimeType: string, base64: string): string {
  return `data:${mimeType};base64,${base64}`;
}

export function countStatuses(
  cases: CaseRecord[],
): Omit<JsonReport, "framework" | "meta" | "partial" | "filtered" | "duration_ms" | "cases"> {
  const counts = { total: cases.length, passed: 0, failed: 0, skipped: 0, xfail: 0, xpass: 0, timeout: 0 };
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

/* ------------------------------------------------------------------ */
/* assets                                                              */
/* ------------------------------------------------------------------ */

const STYLE = `
/* Light is the base palette; dark redefines only tokens, once for the
   un-stamped system default and once for an explicit toggle. Every component
   below reads tokens, so no colour is ever defined only inside a guard. */
:root {
  color-scheme: light dark;
  --bg:#f5f6f8; --panel:#ffffff; --ink:#161b22; --muted:#5a6572; --line:#dfe4ea;
  --pass:#0f7a53; --fail:#b8202a; --skip:#8a6207; --accent:#3352c9;
  --pass-soft:#e6f4ee; --fail-soft:#fceceb; --skip-soft:#fbf2dd;
  --shadow:0 1px 2px rgba(16,24,40,.06),0 1px 3px rgba(16,24,40,.08);
  --sans: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", sans-serif;
  /* The report opens from disk in CI and must never fetch a font. */
  --mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg:#0f1216; --panel:#171b21; --ink:#e4e8ee; --muted:#8d97a4; --line:#262c34;
    --pass:#49c07d; --fail:#f06a68; --skip:#d3a03a; --accent:#7a9bff;
    --pass-soft:#122318; --fail-soft:#2a1517; --skip-soft:#272013;
    --shadow:0 1px 2px rgba(0,0,0,.45);
  }
}
:root[data-theme="dark"] {
  --bg:#0f1216; --panel:#171b21; --ink:#e4e8ee; --muted:#8d97a4; --line:#262c34;
  --pass:#49c07d; --fail:#f06a68; --skip:#d3a03a; --accent:#7a9bff;
  --pass-soft:#122318; --fail-soft:#2a1517; --skip-soft:#272013;
  --shadow:0 1px 2px rgba(0,0,0,.45);
}
* { box-sizing: border-box; }
body { margin:0; background:var(--bg); color:var(--ink); font:14px/1.55 var(--sans); }
main { max-width:1180px; margin:0 auto; padding:28px 20px 72px; }
a { color:var(--accent); }
.skip-link { position:absolute; left:-9999px; }
.skip-link:focus { left:8px; top:8px; background:var(--panel); padding:8px 12px; border-radius:8px; }

.hero { display:flex; gap:24px; align-items:center; justify-content:space-between; flex-wrap:wrap;
  background:var(--panel); border:1px solid var(--line); border-left:6px solid var(--muted);
  border-radius:14px; padding:22px 24px; box-shadow:var(--shadow); }
.hero.tone-pass { border-left-color:var(--pass); }
.hero.tone-fail { border-left-color:var(--fail); }
.hero.tone-warn { border-left-color:var(--skip); }
.hero-text { min-width:min(100%,320px); flex:1; }
.eyebrow { margin:0; font-size:11px; letter-spacing:.12em; text-transform:uppercase; color:var(--muted); }
h1 { margin:2px 0 4px; font-size:30px; line-height:1.15; letter-spacing:-.02em; text-wrap:balance; }
.tone-pass .verdict { color:var(--pass); }
.tone-fail .verdict { color:var(--fail); }
.tone-warn .verdict { color:var(--skip); }
.lede { margin:0 0 14px; color:var(--muted); }
.chips { display:flex; flex-wrap:wrap; gap:6px; }
.chip { display:inline-flex; gap:6px; align-items:baseline; font-size:12px; padding:3px 9px;
  border:1px solid var(--line); border-radius:999px; font-family:var(--mono); }
.chip b { font-weight:600; color:var(--muted); font-family:inherit; }
.flags { margin-top:10px; display:flex; gap:6px; }
.flag { font-size:11px; text-transform:uppercase; letter-spacing:.08em; padding:3px 9px;
  border-radius:999px; background:var(--skip-soft); color:var(--skip); border:1px solid var(--line); }

.donut { display:flex; align-items:center; gap:14px; }
.donut svg { flex:none; }
.donut-track { fill:none; stroke:var(--line); stroke-width:12; }
.donut-value { fill:none; stroke:var(--pass); stroke-width:12; stroke-linecap:round; }
.tone-fail .donut-value { stroke:var(--fail); }
.tone-warn .donut-value { stroke:var(--skip); }
.donut-label { display:flex; flex-direction:column; }
.donut-label strong { font-size:26px; letter-spacing:-.02em; font-variant-numeric:tabular-nums; }
.donut-label span { font-size:12px; color:var(--muted); }

.metrics { display:grid; grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:12px; margin:16px 0 0; }
.metric { background:var(--panel); border:1px solid var(--line); border-radius:12px; padding:12px 14px; box-shadow:var(--shadow); }
.metric dt { font-size:11px; text-transform:uppercase; letter-spacing:.07em; color:var(--muted); }
.metric dd { margin:2px 0 0; font-size:24px; font-weight:600; letter-spacing:-.02em; font-variant-numeric:tabular-nums; }
.metric small { color:var(--muted); }

.bar { display:flex; height:8px; border-radius:999px; overflow:hidden; margin:18px 0 6px; background:var(--line); }
.seg.pass { background:var(--pass); } .seg.fail { background:var(--fail); } .seg.skip { background:var(--skip); }
.bar-caption { margin:0 0 18px; color:var(--muted); font-size:12px; }

.toolbar { display:flex; gap:10px; flex-wrap:wrap; align-items:center; margin:0 0 18px;
  position:sticky; top:0; z-index:5; padding:10px 0; background:var(--bg); }
.toolbar input { flex:1 1 260px; min-width:200px; padding:9px 12px; border-radius:10px;
  border:1px solid var(--line); background:var(--panel); color:var(--ink); font:inherit; }
.pills { display:flex; gap:6px; flex-wrap:wrap; }
.pill { padding:6px 12px; border-radius:999px; border:1px solid var(--line); background:var(--panel);
  color:var(--ink); font:inherit; font-size:13px; cursor:pointer; }
.pill.pass { color:var(--pass); } .pill.fail { color:var(--fail); } .pill.skip { color:var(--skip); }
.pill[aria-pressed="true"] { background:var(--ink); color:var(--bg); border-color:var(--ink); }
.actions { display:flex; gap:6px; }
.actions button { padding:6px 12px; border-radius:10px; border:1px solid var(--line);
  background:var(--panel); color:var(--muted); font:inherit; font-size:13px; cursor:pointer; }
.actions button:hover, .pill:hover { border-color:var(--accent); }
:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
@media (prefers-reduced-motion: reduce) { * { animation:none !important; transition:none !important; } }

.panel { background:var(--panel); border:1px solid var(--line); border-radius:12px;
  margin:0 0 14px; box-shadow:var(--shadow); }
.panel > summary { cursor:pointer; padding:13px 16px; display:flex; gap:10px; align-items:baseline; }
.panel > summary::-webkit-details-marker { display:none; }
.panel > summary::before { content:"▸"; color:var(--muted); }
.panel[open] > summary::before { content:"▾"; }
.panel-title { font-weight:600; }
.panel-sub { color:var(--muted); font-size:12px; }
.panel-body { padding:0 16px 16px; }
.cover-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(280px,1fr)); gap:14px; }
.cover-group h3 { margin:0 0 6px; font-size:13px; font-family:var(--mono); }
.cover-group h3 small { color:var(--muted); font-weight:400; }
.cover-tags { display:flex; flex-wrap:wrap; gap:4px; }
.cover { font-size:11px; font-family:var(--mono); padding:2px 7px; border-radius:6px; border:1px solid var(--line); }
.cover-ok { background:var(--pass-soft); color:var(--pass); border-color:color-mix(in srgb, var(--pass) 30%, var(--line)); }
.cover-shape { background:transparent; color:var(--pass); border-style:dashed; }
.cover-pending { background:var(--skip-soft); color:var(--skip); }
.cover-none { color:var(--muted); opacity:.6; }
.legend { margin:0 0 14px; padding:0; list-style:none; color:var(--muted); font-size:12px;
  display:flex; gap:6px 18px; flex-wrap:wrap; }
.legend li { display:flex; gap:6px; align-items:center; white-space:nowrap; }
.cover-section { margin:0 0 18px; }
.cover-section-head { font-size:13px; margin:0 0 8px; display:flex; gap:8px; align-items:baseline; flex-wrap:wrap; }
.cover-section-head small { color:var(--muted); font-weight:400; }
table.slowest { width:100%; border-collapse:collapse; }
table.slowest td { padding:5px 8px; border-bottom:1px solid var(--line); }
table.slowest td.num { text-align:right; font-family:var(--mono); white-space:nowrap; }
td.spark { width:40%; }
td.spark span { display:block; height:6px; border-radius:999px; background:var(--accent); opacity:.65; }

.suite { margin:0 0 22px; }
.suite-head { display:flex; justify-content:space-between; gap:12px; align-items:baseline;
  font-size:13px; margin:0 0 8px; padding:0 2px; flex-wrap:wrap; }
.suite-name { font-family:var(--mono); font-weight:600; }
.suite-meta { display:flex; gap:6px; flex-wrap:wrap; }
.mini { font-size:11px; color:var(--muted); padding:2px 7px; border:1px solid var(--line); border-radius:999px; }
.mini.pass { color:var(--pass); } .mini.fail { color:var(--fail); } .mini.skip { color:var(--skip); }

.case { background:var(--panel); border:1px solid var(--line); border-radius:10px; margin:0 0 8px; }
.case.fail { border-color:color-mix(in srgb, var(--fail) 45%, var(--line)); }
.case > summary { display:flex; gap:10px; align-items:center; cursor:pointer; padding:11px 14px; list-style:none; }
.case > summary::-webkit-details-marker { display:none; }
.case-title { flex:1; font-weight:500; }
.case-time { color:var(--muted); font-family:var(--mono); font-size:12px; font-variant-numeric:tabular-nums; }
.badge { font-size:11px; text-transform:uppercase; letter-spacing:.05em; padding:2px 8px;
  border-radius:6px; white-space:nowrap; }
.badge.pass { background:var(--pass-soft); color:var(--pass); }
.badge.fail { background:var(--fail-soft); color:var(--fail); }
.badge.skip { background:var(--skip-soft); color:var(--skip); }
.badge.sm { font-size:10px; padding:1px 6px; }
.body { padding:0 14px 14px; border-top:1px solid var(--line); }
.meta { color:var(--muted); font-size:12px; margin:10px 0; }
.covers { display:flex; flex-wrap:wrap; gap:4px; margin:0 0 10px; }
.tag { font-size:11px; font-family:var(--mono); padding:2px 7px; border-radius:6px;
  background:var(--bg); border:1px solid var(--line); color:var(--muted); }
.reason { margin:0 0 10px; padding:9px 12px; border-left:3px solid var(--skip);
  background:var(--skip-soft); border-radius:0 8px 8px 0; }
.failure { border:1px solid color-mix(in srgb, var(--fail) 35%, var(--line));
  background:var(--fail-soft); border-radius:10px; padding:12px 14px; margin:0 0 12px; }
.failure h3 { margin:0 0 8px; font-size:13px; color:var(--fail); }
pre { white-space:pre-wrap; word-break:break-word; margin:0; font-family:var(--mono); font-size:12.5px; }
pre.message { background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:10px 12px; }
.compare { display:grid; grid-template-columns:repeat(auto-fit,minmax(220px,1fr)); gap:10px; margin:10px 0 0; }
.side h4 { margin:0 0 4px; font-size:11px; text-transform:uppercase; letter-spacing:.06em; color:var(--muted); }
.side pre { background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:8px 10px; }
.side.expected pre { border-left:3px solid var(--pass); }
.side.actual pre { border-left:3px solid var(--fail); }
.at { margin:8px 0 0; color:var(--muted); font-size:12px; font-family:var(--mono); }
.stack { margin-top:8px; }
.stack summary { cursor:pointer; color:var(--muted); font-size:12px; }
.stack pre { margin-top:6px; background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:10px 12px; max-height:280px; overflow:auto; }

.steps { list-style:none; margin:0 0 10px; padding:0; }
.steps .steps { margin:6px 0 0 18px; padding-left:10px; border-left:1px solid var(--line); }
.step { margin:6px 0; }
.step-head { display:flex; gap:8px; align-items:center; }
.step-name { flex:0 1 auto; }
.step-time { color:var(--muted); font-family:var(--mono); font-size:12px; }
.step-bar { flex:1; height:4px; background:var(--line); border-radius:999px; overflow:hidden; min-width:40px; }
.step-bar span { display:block; height:100%; background:var(--accent); opacity:.55; }

table.assertions { width:100%; border-collapse:collapse; margin:8px 0; font-size:12.5px; display:block; overflow-x:auto; }
table.assertions caption { text-align:left; color:var(--muted); font-size:11px;
  text-transform:uppercase; letter-spacing:.06em; padding-bottom:4px; }
table.assertions th, table.assertions td { text-align:left; vertical-align:top; padding:5px 8px;
  border-bottom:1px solid var(--line); }
table.assertions th { color:var(--muted); font-size:11px; text-transform:uppercase; letter-spacing:.05em; }
table.assertions td { font-family:var(--mono); word-break:break-word; }
table.assertions td:first-child, table.assertions td:nth-child(2) { white-space:nowrap; word-break:normal; }
table.assertions td.pass { color:var(--pass); } table.assertions td.fail { color:var(--fail); }
.row-fail { background:var(--fail-soft); }
.empty { color:var(--muted); margin:8px 0; }
.shot { margin:10px 0 0; }
.shot img { max-width:100%; border-radius:8px; border:1px solid var(--line); display:block; }
.shot figcaption { color:var(--muted); font-size:11px; margin-top:4px; font-family:var(--mono); }
.artifacts { margin-top:12px; }
.artifacts h4 { margin:0 0 6px; font-size:11px; text-transform:uppercase; letter-spacing:.06em; color:var(--muted); }
.artifact { border:1px solid var(--line); border-radius:8px; margin:0 0 6px; background:var(--bg); }
.artifact > summary { cursor:pointer; padding:7px 10px; font-family:var(--mono); font-size:12px; }
.artifact .path, .shot .path { color:var(--muted); font-size:11px; }
.artifact pre { padding:0 10px 10px; max-height:340px; overflow:auto; font-size:12px; }
.artifact-path { color:var(--muted); font-size:11px; font-family:var(--mono); word-break:break-all; margin:0 0 4px; }
.artifact-path b { font-family:inherit; }
.no-match { text-align:center; color:var(--muted); padding:32px 0; }
footer { margin-top:32px; color:var(--muted); font-size:12px; text-align:center; }
@media print {
  .toolbar, .actions { display:none; }
  .case, .panel { break-inside:avoid; }
}
`;

const SCRIPT = `
(function () {
  var root = document.documentElement;
  var stored = null;
  try { stored = localStorage.getItem("lxdev-report-theme"); } catch (e) {}
  if (stored) root.setAttribute("data-theme", stored);
  var themeButton = document.getElementById("theme");
  if (themeButton) themeButton.addEventListener("click", function () {
    var dark = root.getAttribute("data-theme") === "dark"
      || (!root.getAttribute("data-theme") && matchMedia("(prefers-color-scheme: dark)").matches);
    var next = dark ? "light" : "dark";
    root.setAttribute("data-theme", next);
    try { localStorage.setItem("lxdev-report-theme", next); } catch (e) {}
  });

  var filter = "all";
  var query = "";
  var cases = [].slice.call(document.querySelectorAll(".case"));
  var suites = [].slice.call(document.querySelectorAll(".suite"));
  var pills = [].slice.call(document.querySelectorAll("[data-filter]"));
  var covers = [].slice.call(document.querySelectorAll(".cover"));
  var noMatch = document.getElementById("no-match");

  function apply() {
    var needle = query.toLowerCase();
    var shown = 0;
    cases.forEach(function (node) {
      var status = node.getAttribute("data-status");
      var hay = (node.getAttribute("data-search") || "").toLowerCase();
      var statusOk = filter === "all" || status === filter;
      var textOk = !needle || hay.indexOf(needle) !== -1;
      var visible = statusOk && textOk;
      node.hidden = !visible;
      if (visible) shown += 1;
    });
    suites.forEach(function (suite) {
      var any = [].slice.call(suite.querySelectorAll(".case")).some(function (node) { return !node.hidden; });
      suite.hidden = !any;
    });
    covers.forEach(function (node) {
      var hay = (node.getAttribute("data-search") || "").toLowerCase();
      node.style.opacity = !needle || hay.indexOf(needle) !== -1 ? "" : "0.25";
    });
    if (noMatch) noMatch.hidden = shown !== 0;
  }

  pills.forEach(function (pill) {
    pill.addEventListener("click", function () {
      filter = pill.getAttribute("data-filter") || "all";
      pills.forEach(function (other) {
        other.setAttribute("aria-pressed", other === pill ? "true" : "false");
      });
      apply();
    });
  });

  var box = document.getElementById("q");
  if (box) box.addEventListener("input", function (event) {
    query = event.target.value || "";
    apply();
  });
  document.addEventListener("keydown", function (event) {
    if (event.key === "/" && document.activeElement !== box && box) {
      event.preventDefault();
      box.focus();
    }
  });

  function setAll(open) {
    cases.forEach(function (node) { if (!node.hidden) node.open = open; });
  }
  var expand = document.getElementById("expand");
  var collapse = document.getElementById("collapse");
  if (expand) expand.addEventListener("click", function () { setAll(true); });
  if (collapse) collapse.addEventListener("click", function () { setAll(false); });

  if (location.hash) {
    var target = document.getElementById(location.hash.slice(1));
    if (target) target.open = true;
  }
  document.addEventListener("click", function (event) {
    var link = event.target.closest ? event.target.closest("a[href^='#case-']") : null;
    if (!link) return;
    var target = document.getElementById(link.getAttribute("href").slice(1));
    if (target) target.open = true;
  });
  apply();
})();
`;
