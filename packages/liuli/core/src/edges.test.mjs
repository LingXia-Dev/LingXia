import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { test } from 'node:test';

// Edges come in two roles, which high contrast takes to 3:1 and 4.5:1: `line` (dividers, container and
// control edges) and `line-strong` (field boundaries). Only a disabled look keeps a quiet `text/8` or `text/12`.
const tiers = new Set(['line', 'line-strong']);
const quiet = new Set(['text/8', 'text/12']);
const other = /^(accent|focus|danger|warning|success|transparent|on-media|on-accent)/;
const disabledKey = /^\s*(disabled|offDisabled|off)\s*:/;

test('recipe edges use the line roles; only disabled looks stay quiet', () => {
  const dir = new URL('.', import.meta.url);
  const stray = [];
  for (const name of readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name)) {
    const file = new URL(`${name}/${name}.recipe.ts`, dir);
    let source;
    try { source = readFileSync(file, 'utf8'); } catch { continue; }
    for (const line of source.split('\n')) {
      for (const [, prefix, colour] of line.matchAll(/([a-z-:]*)(?:border|divide)(?:-[xytblrse])?-((?:text|line)[a-z/0-9-]*)/g)) {
        if (tiers.has(colour) || other.test(colour) || prefix.includes('hover:')) continue;
        if (quiet.has(colour) && (prefix.includes('disabled:') || disabledKey.test(line))) continue;
        stray.push(`${name}: ${prefix}${colour}`);
      }
    }
  }
  assert.deepEqual(stray, []);
});
