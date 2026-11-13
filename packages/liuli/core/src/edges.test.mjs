import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { test } from 'node:test';

// Edges come in three strengths: subtle `text/8` (dividers, disabled fields), default `text/12`
// (container and control edges), strong `line-strong` (field boundaries, 3:1). Hover may firm an edge.
const tiers = new Set(['text/8', 'text/12', 'line-strong']);
const other = /^(accent|focus|danger|warning|success|transparent|on-media|on-accent)/;

test('recipe edges use the three strengths', () => {
  const dir = new URL('.', import.meta.url);
  const stray = [];
  for (const name of readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name)) {
    const file = new URL(`${name}/${name}.recipe.ts`, dir);
    let source;
    try { source = readFileSync(file, 'utf8'); } catch { continue; }
    for (const [, prefix, colour] of source.matchAll(/([a-z-:]*)(?:border|divide)(?:-[xytblrse])?-((?:text|line)[a-z/0-9-]*)/g))
      if (!tiers.has(colour) && !other.test(colour) && !prefix.includes('hover:')) stray.push(`${name}: ${prefix}${colour}`);
  }
  assert.deepEqual(stray, []);
});
