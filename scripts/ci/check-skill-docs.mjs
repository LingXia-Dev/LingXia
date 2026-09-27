#!/usr/bin/env node
// Keep the agent skill self-contained and its links working.
//
// `lingxia skill install` copies docs/skill alone, so a skill doc that points
// outside that tree hands the agent a dead reference. Every relative link must
// resolve to a file inside docs/skill, every #anchor to a heading there, and no
// doc may name a repository path (docs/internal, tools/, crates/, ...) as
// reading material. Public https:// links are allowed.
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const root = path.resolve(process.argv[2] ?? 'docs/skill');
const REPO_PATH = /(^|[\s`'"(\[])(?:\.\.\/)*(docs|tools|crates|packages|examples|scripts|website|lingxia-sdk|\.github)\/[\w.-]/;

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return walk(full);
    return entry.name.endsWith('.md') ? [full] : [];
  });
}

// GitHub heading slugs: strip code ticks and link targets, drop punctuation,
// spaces to hyphens, number repeats.
function slug(text) {
  return text
    .replace(/`/g, '')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}_\- ]/gu, '')
    .replace(/ /g, '-');
}

const anchorCache = new Map();
function anchors(file) {
  if (anchorCache.has(file)) return anchorCache.get(file);
  const seen = new Map();
  const out = new Set();
  let fence = false;
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) { fence = !fence; continue; }
    if (fence) continue;
    const m = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (!m) continue;
    const s = slug(m[2]);
    const n = seen.get(s) ?? 0;
    seen.set(s, n + 1);
    out.add(n === 0 ? s : `${s}-${n}`);
  }
  anchorCache.set(file, out);
  return out;
}

const problems = [];
const files = walk(root);
for (const file of files) {
  const rel = path.relative(process.cwd(), file);
  let fence = false;
  fs.readFileSync(file, 'utf8').split('\n').forEach((line, i) => {
    const where = `${rel}:${i + 1}`;
    if (/^\s*(```|~~~)/.test(line)) { fence = !fence; return; }
    if (REPO_PATH.test(line)) problems.push(`${where}: names a repository path outside the skill`);
    if (fence) return;
    const prose = line.replace(/`[^`]*`/g, (code) => (code.includes('](') ? code : '``'));
    for (const [, target] of prose.matchAll(/(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
      if (/^[a-z][a-z0-9+.-]*:/i.test(target)) {
        if (!/^(https|mailto):/i.test(target)) problems.push(`${where}: non-https link ${target}`);
        continue;
      }
      const [p, frag] = target.split('#');
      const dest = p ? path.resolve(path.dirname(file), p) : file;
      if (dest !== root && !dest.startsWith(root + path.sep)) {
        problems.push(`${where}: link leaves the skill: ${target}`);
        continue;
      }
      if (!fs.existsSync(dest)) { problems.push(`${where}: missing file ${target}`); continue; }
      if (frag && dest.endsWith('.md') && !anchors(dest).has(frag)) {
        problems.push(`${where}: missing anchor ${target}`);
      }
    }
  });
}

for (const p of problems) console.error(p);
console.log(`${files.length} skill doc(s), ${problems.length} problem(s)`);
process.exit(problems.length ? 1 : 0);
