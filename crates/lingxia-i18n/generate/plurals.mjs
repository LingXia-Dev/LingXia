// Generates CLDR cardinal and ordinal plural rules for lingxia-i18n: one Rust function per distinct
// rule set, the same rule as a JS arrow, and the language → rule maps. Source: cldr-core (pinned).
// node plurals.mjs (from this directory): writes ../src/plural/rules.rs and ../tests/plural-cases.json
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const cldrVersion = require('cldr-core/package.json').version;
const cardinal = require('cldr-core/supplemental/plurals.json').supplemental['plurals-type-cardinal'];
const ordinal = require('cldr-core/supplemental/ordinals.json').supplemental['plurals-type-ordinal'];
const outRs = new URL("../src/plural/rules.rs", import.meta.url), outCases = new URL("../tests/plural-cases.json", import.meta.url);

const CATEGORIES = ['zero', 'one', 'two', 'few', 'many', 'other'];
const RUST_CATEGORY = { zero: 'Zero', one: 'One', two: 'Two', few: 'Few', many: 'Many', other: 'Other' };

/** A rule set as [[category, condition]], `other` last and implicit. */
function ruleSet(rules) {
  return CATEGORIES.filter((c) => c !== 'other' && rules[`pluralRule-count-${c}`] !== undefined)
    .map((c) => [c, rules[`pluralRule-count-${c}`].split('@')[0].trim()]);
}

// CLDR condition grammar: or-list of and-lists of `expr (=|!=) range_list`, expr = operand [% mod].
function parse(condition) {
  if (condition === '') return null;
  return condition.split(/\s+or\s+/).map((and) => and.split(/\s+and\s+/).map((relation) => {
    const m = relation.match(/^([niftvwec])\s*(?:%\s*(\d+))?\s*(!=|=)\s*(.+)$/);
    if (!m) throw new Error(`unparsed relation: ${relation}`);
    const [, operand, mod, op, list] = m;
    const ranges = list.split(',').map((part) => {
      const [lo, hi] = part.trim().split('..').map(Number);
      return [lo, hi ?? lo];
    });
    return { operand: operand === 'c' ? 'e' : operand, mod: mod ? Number(mod) : null, negate: op === '!=', ranges };
  }));
}

// `&&` binds tighter than `||`, so only a relation that is itself a list needs parentheses.
const group = (s) => (/&&|\|\|/.test(s) ? `(${s})` : s);

function js(tree) {
  return tree.map((and) => and.map(({ operand, mod, negate, ranges }) => {
    const x = mod === null ? `o.${operand}` : `o.${operand} % ${mod}`;
    const terms = ranges.map(([lo, hi]) => {
      if (lo === hi) return `${x} ${negate ? '!==' : '==='} ${lo}`;
      // A range only matches whole values; `n` may carry a fraction.
      const within = `${operand === 'n' ? `${x} % 1 === 0 && ` : ''}${x} >= ${lo} && ${x} <= ${hi}`;
      return negate ? `!(${within})` : within;
    });
    return group(terms.map(ranges.length > 1 ? group : (t) => t).join(negate ? ' && ' : ' || '));
  }).join(' && ')).join(' || ');
}

function rust(tree) {
  return tree.map((and) => and.map(({ operand, mod, negate, ranges }) => {
    const float = operand === 'n';
    const x = mod === null ? `o.${operand}` : `o.${operand} % ${mod}${float ? '.0' : ''}`;
    const lit = (n) => (float ? `${n}.0` : `${n}`);
    // `!=` is "in none of the ranges", written out so clippy sees no double negation.
    const terms = ranges.map(([lo, hi]) => {
      if (lo === 0 && hi === 0 && mod !== null && !float) {
        return `${negate ? '!' : ''}${x.split(' ')[0]}.is_multiple_of(${mod})`;
      }
      if (lo === hi) return `${x} ${negate ? '!=' : '=='} ${lit(lo)}`;
      const operand = x.includes(' ') ? `(${x})` : x;
      const within = `(${lit(lo)}..=${lit(hi)}).contains(&${operand})`;
      // A range only matches whole values; `n` may carry a fraction.
      const test = float ? `${operand}.fract() == 0.0 && ${within}` : within;
      return negate ? (float ? `!(${test})` : `!${test}`) : test;
    });
    return group(terms.map(ranges.length > 1 ? group : (s) => s).join(negate ? ' && ' : ' || '));
  }).join(' && ')).join(' || ');
}

const rules = []; // distinct rule sets, shared by cardinal and ordinal
const keyOf = new Map();
function ruleIndex(set) {
  const key = JSON.stringify(set);
  if (!keyOf.has(key)) { keyOf.set(key, rules.length); rules.push(set); }
  return keyOf.get(key);
}
const languages = (table) => Object.keys(table).filter((l) => l !== 'root').sort();
const cardinalMap = languages(cardinal).map((l) => [l, ruleIndex(ruleSet(cardinal[l]))]);
const ordinalMap = languages(ordinal).map((l) => [l, ruleIndex(ruleSet(ordinal[l]))]);

// The whole condition needs no outer parentheses.
const bare = (s) => {
  if (!s.startsWith('(')) return s;
  let depth = 0;
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '(') depth++;
    else if (s[i] === ')' && --depth === 0 && i < s.length - 1) return s;
  }
  return s.slice(1, -1);
};
const jsRule = (set) => {
  const cases = set.map(([c, cond]) => `if (${bare(js(parse(cond)))}) return '${c}'; `).join('');
  return `o => { ${cases}return 'other'; }`;
};
const rustRule = (set, i) => {
  const body = set.map(([c, cond]) => `    if ${bare(rust(parse(cond)))} {\n        return PluralCategory::${RUST_CATEGORY[c]};\n    }\n`).join('');
  return `fn r${i}(${set.length ? 'o' : '_o'}: &Operands) -> PluralCategory {\n${body}    PluralCategory::Other\n}\n`;
};
const arms = (map) => {
  const groups = new Map();
  for (const [l, i] of map) groups.set(i, [...(groups.get(i) ?? []), l]);
  return [...groups].map(([i, ls]) => `        ${ls.map((l) => JSON.stringify(l)).join(' | ')} => ${i},`).join('\n');
};

// The operands of a number as JS shows it; plural.rs computes the same from Rust's shortest digits.
const OPERANDS = `function operands(value) {
  const n = Math.abs(value);
  let text = String(n);
  // Only the fraction is read: a huge number has none, a tiny one is written out (old WebViews: no ?? or BigInt).
  if (text.indexOf('e') >= 0) text = n >= 1 ? '0' : n.toFixed(20).replace(/0+$/, '');
  const fraction = text.split('.')[1] || '';
  const trimmed = fraction.replace(/0+$/, '');
  return { n, i: Math.trunc(n), v: fraction.length, w: trimmed.length, f: Number(fraction.slice(0, 18) || 0), t: Number(trimmed.slice(0, 18) || 0), e: 0 };
}`;
const operands = new Function(`${OPERANDS}; return operands;`)();

const out = `// @generated by crates/lingxia-i18n/generate/plurals.mjs from cldr-core ${cldrVersion}; do not edit.
// Rule \`r<k>\` and \`JS_RULES[k]\` are the same CLDR rule set.

use crate::message::PluralCategory;
use super::Operands;

pub const CLDR_VERSION: &str = ${JSON.stringify(cldrVersion)};

/// The rule set a language's cardinal plurals follow, by its CLDR language id.
pub(crate) fn cardinal_rule(language: &str) -> Option<usize> {
    Some(match language {
${arms(cardinalMap)}
        _ => return None,
    })
}

/// The rule set a language's ordinals follow.
pub(crate) fn ordinal_rule(language: &str) -> Option<usize> {
    Some(match language {
${arms(ordinalMap)}
        _ => return None,
    })
}

pub(crate) fn apply(rule: usize, o: &Operands) -> PluralCategory {
    RULES[rule](o)
}

const RULES: [fn(&Operands) -> PluralCategory; ${rules.length}] = [${rules.map((_, i) => `r${i}`).join(', ')}];

/// \`operands(value)\` in JS, for the rules below.
pub(crate) const JS_OPERANDS: &str = ${JSON.stringify(OPERANDS)};

/// Each rule as a JS function of the same operands, for the compiled catalogue's \`f.p\`.
pub(crate) const JS_RULES: [&str; ${rules.length}] = [
${rules.map((set) => `    ${JSON.stringify(jsRule(set))},`).join('\n')}
];

${rules.map(rustRule).join('\n')}`;
fs.writeFileSync(outRs, out);
// Committed as rustfmt leaves it, so `cargo fmt --check` and a fresh generation agree.
execFileSync('rustfmt', ['--edition', '2024', fileURLToPath(outRs)]);

// Test cases: CLDR's own samples for each category, judged by Node's Intl (ICU) as an independent check.
const LANGS = ['de', 'es', 'he', 'ru', 'ar', 'zh', 'en', 'fr', 'pt', 'ja', 'ko'];
const samples = (text) => {
  const out = [];
  for (const part of (text.split('@integer')[1] ?? '').split('@decimal')[0].split(',')) {
    const t = part.trim();
    if (!t || t === '…' || /c/.test(t)) continue;
    const [lo, hi] = t.split('~');
    if (!hi) out.push(lo); else for (let n = Number(lo), k = 0; n <= Number(hi) && k < 3; n++, k++) out.push(String(n));
  }
  for (const part of (text.split('@decimal')[1] ?? '').split(',')) {
    const t = part.trim();
    if (!t || t === '…' || /c/.test(t)) continue;
    out.push(t.split('~')[0]);
  }
  return out;
};
const cases = [];
for (const [type, table] of [['cardinal', cardinal], ['ordinal', ordinal]]) {
  for (const lang of LANGS) {
    const pr = new Intl.PluralRules(lang, { type });
    for (const c of CATEGORIES) {
      const text = table[lang]?.[`pluralRule-count-${c}`];
      if (text === undefined) continue;
      for (const s of samples(text)) {
        // A sample whose visible trailing zeros JS numbers cannot carry (1.0) is not a case for f.p(number).
        if (String(Number(s)) !== s) continue;
        const icu = pr.select(Number(s));
        if (icu !== c) throw new Error(`${lang} ${type} ${s}: CLDR says ${c}, ICU says ${icu}`);
        cases.push({ language: lang, type, value: Number(s), category: c });
      }
    }
  }
}
// The JS rules, run on the shared cases: the Rust side runs the same table in its unit test.
const jsRules = rules.map((set) => new Function(`return ${jsRule(set)}`)());
for (const c of cases) {
  const index = (c.type === 'ordinal' ? ordinalMap : cardinalMap).find(([l]) => l === c.language)[1];
  const got = jsRules[index](operands(c.value));
  if (got !== c.category) throw new Error(`JS rule: ${c.language} ${c.type} ${c.value} gave ${got}, CLDR says ${c.category}`);
}
fs.writeFileSync(outCases, JSON.stringify({ cldr: cldrVersion, cases }, null, 1) + '\n');
console.log(JSON.stringify({ cldr: cldrVersion, cardinalLanguages: cardinalMap.length, ordinalLanguages: ordinalMap.length, distinctRules: rules.length, cases: cases.length }));
