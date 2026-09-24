import type { CaseRecord, SpecStatus, TagSummary } from "./types.js";

/**
 * A tag names a layer or a slice of the suite (`unit`, `routed`, `live`,
 * `smoke`). Letters, digits and `_ . : / -`, starting with a letter or
 * digit, so a tag never collides with the `!` and `,` of a selection.
 */
const TAG = /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/;

/** The report's bucket for specs without any tag; not a valid tag itself. */
export const UNTAGGED = "(untagged)";

export function validateTags(tags: unknown, where: string): string[] {
  if (tags === undefined) return [];
  if (!Array.isArray(tags)) throw new TypeError(`${where} tags must be an array of strings`);
  const out: string[] = [];
  for (const tag of tags) {
    if (typeof tag !== "string" || !TAG.test(tag)) {
      throw new TypeError(
        `${where} tag ${JSON.stringify(tag)} is invalid: use letters, digits and _ . : / -, starting with a letter or digit`,
      );
    }
    if (!out.includes(tag)) out.push(tag);
  }
  return out;
}

/** One `--tag` value: any of its terms; `!tag` is "does not have tag". */
export interface TagClause {
  source: string;
  terms: Array<{ tag: string; negated: boolean }>;
}

/**
 * Parse `--tag` values. Each value is a clause whose comma-separated terms
 * are alternatives (`a,b` = has a or b; `!a` = has no a); several clauses
 * must all hold. Throws on an empty term or an invalid tag.
 */
export function parseTagFilter(values: readonly string[]): TagClause[] {
  return values.map((source) => {
    const terms = source.split(",").map((raw) => {
      const term = raw.trim();
      const negated = term.startsWith("!");
      const tag = negated ? term.slice(1).trim() : term;
      if (!TAG.test(tag)) {
        throw new Error(
          `--tag ${JSON.stringify(source)}: ${JSON.stringify(term)} is not a tag or !tag`,
        );
      }
      return { tag, negated };
    });
    return { source, terms };
  });
}

/**
 * Whether a spec with `tags` passes every clause. A spec without tags has
 * none of them: it fails `--tag routed` and passes `--tag '!live'`.
 */
export function matchesTags(tags: readonly string[], clauses: readonly TagClause[]): boolean {
  return clauses.every((clause) =>
    clause.terms.some((term) => tags.includes(term.tag) !== term.negated));
}

/** Specs with tag X, by status: a failing `live` group cannot hide a clean `routed` one. */
export function tagSummary(cases: readonly CaseRecord[]): TagSummary[] {
  const buckets = new Map<string, TagSummary>();
  const touch = (tag: string): TagSummary => {
    let entry = buckets.get(tag);
    if (!entry) {
      entry = { tag, total: 0, passed: 0, failed: 0, skipped: 0, timeout: 0, xfail: 0, xpass: 0, flaky: 0, ok: true };
      buckets.set(tag, entry);
    }
    return entry;
  };
  const anyTagged = cases.some((item) => (item.tags ?? []).length > 0);
  if (!anyTagged) return [];
  for (const item of cases) {
    const tags = item.tags && item.tags.length > 0 ? item.tags : [UNTAGGED];
    for (const tag of tags) {
      const entry = touch(tag);
      entry.total += 1;
      entry[item.status as SpecStatus] += 1;
      if (item.flaky) entry.flaky += 1;
      if (item.status === "failed" || item.status === "timeout" || item.status === "xpass") entry.ok = false;
    }
  }
  return [...buckets.values()].sort((left, right) =>
    left.tag === UNTAGGED ? 1 : right.tag === UNTAGGED ? -1 : left.tag.localeCompare(right.tag));
}
