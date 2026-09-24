import type { CaseRecord, CoverageSpec, CoverageSummary, SpecStatus } from "./types.js";

/** One requirement of a `--covers-manifest` file, as lxdev sends it. */
export interface ManifestEntry {
  id: string;
  title?: string;
}

/** A registered spec, whether or not this selection ran it. */
export interface RegisteredCover {
  id: string;
  title: string;
  covers: readonly string[];
}

/**
 * `control.coversManifest` is a JSON list of `{ id, title? }`; lxdev has
 * already read and checked the file. Anything else is a broken control.
 */
export function parseManifest(raw: string | undefined): ManifestEntry[] | undefined {
  if (raw === undefined) return undefined;
  const parsed: unknown = JSON.parse(raw);
  if (!Array.isArray(parsed)) throw new Error("coversManifest control must be a JSON list");
  const seen = new Set<string>();
  return parsed.map((entry, index) => {
    const record = (entry && typeof entry === "object" ? entry : {}) as { id?: unknown; title?: unknown };
    if (typeof record.id !== "string" || record.id.length === 0) {
      throw new Error(`coversManifest entry ${index} has no id`);
    }
    if (seen.has(record.id)) throw new Error(`coversManifest lists ${JSON.stringify(record.id)} twice`);
    seen.add(record.id);
    return typeof record.title === "string" ? { id: record.id, title: record.title } : { id: record.id };
  });
}

const BROKEN: ReadonlySet<SpecStatus> = new Set(["failed", "timeout", "xpass"]);

/**
 * Which manifest ids the suite covers, by which specs and with what outcome;
 * which have no spec at all; and which `covers` ids are not in the manifest.
 * A spec registered but outside this selection still covers its ids, as
 * `not_run`, so a filtered run never reports a false hole.
 */
export function coverageSummary(
  manifest: readonly ManifestEntry[],
  cases: readonly CaseRecord[],
  registered: readonly RegisteredCover[],
): CoverageSummary {
  const ran = new Map<string, CaseRecord[]>();
  for (const item of cases) {
    const list = ran.get(item.id) ?? [];
    list.push(item);
    ran.set(item.id, list);
  }
  const specsFor = (id: string): CoverageSpec[] =>
    registered
      .filter((spec) => spec.covers.includes(id))
      .map((spec) => ({ id: spec.id, title: spec.title, status: outcome(ran.get(spec.id)) }));
  const known = new Set(manifest.map((entry) => entry.id));
  const ids = manifest.map((entry) => {
    const specs = specsFor(entry.id);
    return { ...entry, status: rollUp(specs), specs };
  });
  const unknown = new Map<string, string[]>();
  for (const spec of registered) {
    for (const id of spec.covers) {
      if (known.has(id)) continue;
      const list = unknown.get(id) ?? [];
      if (!list.includes(spec.id)) list.push(spec.id);
      unknown.set(id, list);
    }
  }
  const covered = ids.filter((entry) => entry.specs.length > 0);
  return {
    total: manifest.length,
    covered: covered.length,
    passing: covered.filter((entry) => entry.status === "passed").length,
    failing: covered.filter((entry) => entry.status === "failed").length,
    uncovered: ids.filter((entry) => entry.specs.length === 0).map(({ id, title }) => (title ? { id, title } : { id })),
    unknown: [...unknown.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([id, specs]) => ({ id, specs })),
    ids,
  };
}

/** A spec's outcome across its executions (repeats): any break wins. */
function outcome(executions: CaseRecord[] | undefined): CoverageSpec["status"] {
  if (!executions || executions.length === 0) return "not_run";
  const broken = executions.find((item) => BROKEN.has(item.status));
  if (broken) return broken.status;
  if (executions.some((item) => item.status === "passed")) return "passed";
  return executions[0]!.status;
}

/** An id is `failed` if any spec broke, `passed` if one passed, else what is left. */
function rollUp(specs: CoverageSpec[]): CoverageSummary["ids"][number]["status"] {
  if (specs.length === 0) return "uncovered";
  if (specs.some((spec) => BROKEN.has(spec.status as SpecStatus))) return "failed";
  if (specs.some((spec) => spec.status === "passed")) return "passed";
  if (specs.some((spec) => spec.status === "xfail")) return "xfail";
  if (specs.some((spec) => spec.status === "skipped")) return "skipped";
  return "not_run";
}
