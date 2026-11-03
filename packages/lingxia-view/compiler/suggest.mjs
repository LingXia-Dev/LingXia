import { __unstable__loadDesignSystem } from 'tailwindcss';
import { readFile } from 'node:fs/promises';
import { compileUtilities } from './tailwind.mjs';

// Restricted Damerau-Levenshtein: typos are mostly swaps, drops and stray characters.
function distance(a, b, limit) {
  if (Math.abs(a.length - b.length) > limit) return limit + 1;
  let previous2 = [], previous = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const row = [i];
    let best = i;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let value = Math.min(previous[j] + 1, row[j - 1] + 1, previous[j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) value = Math.min(value, previous2[j - 2] + 1);
      row.push(value);
      best = Math.min(best, value);
    }
    if (best > limit) return limit + 1;
    [previous2, previous] = [previous, row];
  }
  return previous[b.length];
}

/**
 * For each unknown class, the closest utility the native renderer can draw,
 * keeping the variant prefix. Names with no candidate within a few edits map to undefined.
 */
export async function nearestSupported(names, theme = '') {
  if (!names.length) return new Map();
  const defaults = await readFile(new URL(import.meta.resolve('tailwindcss/theme.css')), 'utf8');
  const system = await __unstable__loadDesignSystem(`${defaults}\n${theme}`);
  const utilities = system.getClassList().map(([name]) => name);
  const result = new Map();
  for (const name of names) {
    const cut = name.lastIndexOf(':') + 1;
    const [prefix, base] = [name.slice(0, cut), name.slice(cut)];
    const limit = Math.max(2, Math.floor(base.length / 4));
    const ranked = utilities
      .map(candidate => [candidate, distance(base, candidate, limit)])
      .filter(([, score]) => score <= limit)
      .sort((a, b) => a[1] - b[1] || a[0].length - b[0].length)
      .slice(0, 8)
      .map(([candidate]) => candidate);
    // Only name a form the renderer accepts.
    const rules = ranked.length ? await compileUtilities(new Set(ranked), theme) : [];
    const failing = new Set(rules.filter(rule => rule.error).map(rule => rule.name));
    const found = ranked.find(candidate => rules.some(rule => rule.name === candidate) && !failing.has(candidate));
    result.set(name, found && `${prefix}${found}`);
  }
  return result;
}
