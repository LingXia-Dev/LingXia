import { cx, type Child } from '@lingxia/view';
import { mathRecipe as m } from './MathView.recipe';

const SYMBOLS: Readonly<Record<string, string>> = {
  // Greek lowercase
  alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ', epsilon: 'ε', varepsilon: 'ε',
  zeta: 'ζ', eta: 'η', theta: 'θ', vartheta: 'ϑ', iota: 'ι', kappa: 'κ', lambda: 'λ',
  mu: 'μ', nu: 'ν', xi: 'ξ', pi: 'π', varpi: 'ϖ', rho: 'ρ', varrho: 'ϱ',
  sigma: 'σ', varsigma: 'ς', tau: 'τ', upsilon: 'υ', phi: 'φ', varphi: 'φ',
  chi: 'χ', psi: 'ψ', omega: 'ω',
  // Greek uppercase
  Gamma: 'Γ', Delta: 'Δ', Theta: 'Θ', Lambda: 'Λ', Xi: 'Ξ', Pi: 'Π',
  Sigma: 'Σ', Upsilon: 'Υ', Phi: 'Φ', Psi: 'Ψ', Omega: 'Ω',
  // Operators & relations
  infty: '∞', pm: '±', mp: '∓', times: '×', div: '÷', cdot: '·', ast: '∗', star: '⋆', circ: '∘', bullet: '•',
  oplus: '⊕', otimes: '⊗', ominus: '⊖',
  le: '≤', leq: '≤', ge: '≥', geq: '≥', ne: '≠', neq: '≠', approx: '≈',
  equiv: '≡', cong: '≅', sim: '∼', simeq: '≃', propto: '∝', ll: '≪', gg: '≫',
  // Arrows
  to: '→', rightarrow: '→', leftarrow: '←', leftrightarrow: '↔',
  Rightarrow: '⇒', Leftarrow: '⇐', Leftrightarrow: '⇔', implies: '⇒', iff: '⇔',
  mapsto: '↦', uparrow: '↑', downarrow: '↓', rightleftharpoons: '⇌',
  // Set & logic
  in: '∈', notin: '∉', ni: '∋', subset: '⊂', supset: '⊃', subseteq: '⊆', supseteq: '⊇',
  cup: '∪', cap: '∩', setminus: '∖', emptyset: '∅', varnothing: '∅',
  forall: '∀', exists: '∃', nexists: '∄', neg: '¬', lnot: '¬', land: '∧', wedge: '∧', lor: '∨', vee: '∨',
  therefore: '∴', because: '∵',
  // Geometry & physics
  angle: '∠', measuredangle: '∡', triangle: '△', perp: '⊥', parallel: '∥', degree: '°',
  partial: '∂', nabla: '∇', hbar: 'ℏ', ell: 'ℓ', aleph: 'ℵ', Re: 'ℜ', Im: 'ℑ',
  // Dots & spacing
  ldots: '…', dots: '…', cdots: '⋯', vdots: '⋮', ddots: '⋱',
  quad: '  ', qquad: '    ',
  // Big ops fallback
  sum: '∑', prod: '∏', coprod: '∐', int: '∫', iint: '∬', iiint: '∭', oint: '∮',
  bigcup: '⋃', bigcap: '⋂',
};

const BLACKBOARD: Readonly<Record<string, string>> = {
  A: '𝔸', B: '𝔹', C: 'ℂ', D: '𝔻', E: '𝔼', F: '𝔽', G: '𝔾', H: 'ℍ',
  I: '𝕀', J: '𝕁', K: '𝕂', L: '𝕃', M: '𝕄', N: 'ℕ', O: '𝕆', P: 'ℙ',
  Q: 'ℚ', R: 'ℝ', S: '𝕊', T: '𝕋', U: '𝕌', V: '𝕍', W: '𝕎', X: '𝕏',
  Y: '𝕐', Z: 'ℤ',
};

const UPRIGHT_FUNCS = new Set([
  'sin', 'cos', 'tan', 'cot', 'sec', 'csc', 'arcsin', 'arccos', 'arctan',
  'sinh', 'cosh', 'tanh', 'coth', 'log', 'ln', 'lg', 'exp', 'det', 'dim',
  'gcd', 'lcm', 'Pr', 'deg', 'ker', 'hom', 'arg', 'mod', 'bmod',
]);

const LIMIT_FUNCS = new Set(['lim', 'limsup', 'liminf', 'max', 'min', 'sup', 'inf', 'argmax', 'argmin']);
const BIG_OPS = new Set(['int', 'iint', 'iiint', 'oint', 'sum', 'prod', 'coprod', 'bigcup', 'bigcap']);

const ACCENTS: Readonly<Record<string, string>> = {
  hat: '\u0302',
  widehat: '\u0302',
  bar: '\u0304',
  tilde: '\u0303',
  widetilde: '\u0303',
  vec: '\u20D7',
  dot: '\u0307',
  ddot: '\u0308',
};

const SUPER_MAP: Readonly<Record<string, string>> = {
  '0': '⁰', '1': '¹', '2': '²', '3': '³', '4': '⁴', '5': '⁵', '6': '⁶', '7': '⁷', '8': '⁸', '9': '⁹',
  '+': '⁺', '-': '⁻', '−': '⁻', '=': '⁼', '(': '⁽', ')': '⁾',
  a: 'ᵃ', b: 'ᵇ', c: 'ᶜ', d: 'ᵈ', e: 'ᵉ', f: 'ᶠ', g: 'ᵍ', h: 'ʰ', i: 'ⁱ', j: 'ʲ',
  k: 'ᵏ', l: 'ˡ', m: 'ᵐ', n: 'ⁿ', o: 'ᵒ', p: 'ᵖ', r: 'ʳ', s: 'ˢ', t: 'ᵗ', u: 'ᵘ',
  v: 'ᵛ', w: 'ʷ', x: 'ˣ', y: 'ʸ', z: 'ᶻ',
};

const SUB_MAP: Readonly<Record<string, string>> = {
  '0': '₀', '1': '₁', '2': '₂', '3': '₃', '4': '₄', '5': '₅', '6': '₆', '7': '₇', '8': '₈', '9': '₉',
  '+': '₊', '-': '₋', '−': '₋', '=': '₌', '(': '₍', ')': '₎',
  a: 'ₐ', e: 'ₑ', h: 'ₕ', i: 'ᵢ', j: 'ⱼ', k: 'ₖ', l: 'ₗ', m: 'ₘ', n: 'ₙ',
  o: 'ₒ', p: 'ₚ', r: 'ᵣ', s: 'ₛ', t: 'ₜ', u: 'ᵤ', v: 'ᵥ', x: 'ₓ',
};

function mapChars(text: string, table: Readonly<Record<string, string>>): string | null {
  let out = '';
  for (const ch of text) {
    if (ch === ' ') { out += ' '; continue; }
    const mapped = table[ch];
    if (!mapped) return null;
    out += mapped;
  }
  return out;
}

/** Formats `\ce{...}` chemical formulas & equations (e.g. `2H2 + O2 -> 2H2O`, `Fe^2+`, `H2SO4`). */
function formatChemistry(raw: string): string {
  let s = raw
    .replace(/<=>/g, ' ⇌ ')
    .replace(/->/g, ' → ')
    .replace(/<-/g, ' ← ');
  // Charges like ^2+, ^-, ^{2-}
  s = s.replace(/\^\{([^}]+)\}|\^([0-9]*[+-])/g, (_m, g1: string | undefined, g2: string | undefined) => {
    const body = g1 ?? g2 ?? '';
    return mapChars(body, SUPER_MAP) ?? `^${body}`;
  });
  // Stoichiometric subscripts after element symbol or closing paren: H2 -> H₂, (OH)2 -> (OH)₂
  s = s.replace(/([A-Za-z)\]])([0-9]+)/g, (_m, prev: string, digits: string) => {
    return prev + (mapChars(digits, SUB_MAP) ?? digits);
  });
  return s.replace(/\s{2,}/g, ' ').trim();
}

type MathNode =
  | { readonly kind: 'text'; readonly value: string; readonly italic?: boolean; readonly bold?: boolean }
  | { readonly kind: 'op'; readonly value: string }
  | { readonly kind: 'frac'; readonly num: readonly MathNode[]; readonly den: readonly MathNode[]; readonly noBar?: boolean }
  | { readonly kind: 'sqrt'; readonly index?: readonly MathNode[]; readonly body: readonly MathNode[] }
  | { readonly kind: 'overline' | 'underline'; readonly body: readonly MathNode[] }
  | { readonly kind: 'xarrow'; readonly dir: 'right' | 'left'; readonly above: readonly MathNode[]; readonly below?: readonly MathNode[] }
  | { readonly kind: 'script'; readonly base: MathNode; readonly sup?: readonly MathNode[]; readonly sub?: readonly MathNode[] }
  | { readonly kind: 'bigop'; readonly symbol: string; readonly isTextOp?: boolean; readonly sup?: readonly MathNode[]; readonly sub?: readonly MathNode[] }
  | { readonly kind: 'env'; readonly envName: string; readonly rows: readonly (readonly (readonly MathNode[])[])[] };

class LatexParser {
  private pos = 0;
  private readonly src: string;

  constructor(src: string) {
    this.src = src.trim();
  }

  private eof(): boolean {
    return this.pos >= this.src.length;
  }

  private skipWs(): void {
    while (!this.eof() && /\s/.test(this.src[this.pos])) this.pos += 1;
  }

  private readOptionalBracket(): MathNode[] | undefined {
    this.skipWs();
    if (this.src[this.pos] !== '[') return undefined;
    this.pos += 1;
    const nodes = this.parseSequence(']');
    if (this.src[this.pos] === ']') this.pos += 1;
    return nodes;
  }

  private readGroup(): MathNode[] {
    this.skipWs();
    if (this.src[this.pos] === '{') {
      this.pos += 1;
      const nodes = this.parseSequence('}');
      if (this.src[this.pos] === '}') this.pos += 1;
      return nodes;
    }
    const single = this.parseAtom();
    return single ? [single] : [];
  }

  private readRawBraceText(): string {
    this.skipWs();
    if (this.src[this.pos] !== '{') return '';
    this.pos += 1;
    let depth = 1;
    const start = this.pos;
    while (!this.eof() && depth > 0) {
      if (this.src[this.pos] === '{') depth += 1;
      else if (this.src[this.pos] === '}') depth -= 1;
      if (depth > 0) this.pos += 1;
    }
    const text = this.src.slice(start, this.pos);
    if (this.src[this.pos] === '}') this.pos += 1;
    return text;
  }

  private parseEnvironment(envName: string): MathNode {
    const endTag = `\\end{${envName}}`;
    const endIdx = this.src.indexOf(endTag, this.pos);
    const body = endIdx >= 0 ? this.src.slice(this.pos, endIdx) : this.src.slice(this.pos);
    this.pos = endIdx >= 0 ? endIdx + endTag.length : this.src.length;

    // Split rows by `\\` (respecting brace depth)
    const rawRows: string[] = [];
    let cur = '';
    let depth = 0;
    for (let i = 0; i < body.length; i++) {
      if (body[i] === '{') depth += 1;
      else if (body[i] === '}') depth = Math.max(0, depth - 1);
      if (depth === 0 && body[i] === '\\' && body[i + 1] === '\\') {
        rawRows.push(cur.trim());
        cur = '';
        i += 1;
        continue;
      }
      cur += body[i];
    }
    if (cur.trim().length > 0) rawRows.push(cur.trim());

    const rows = rawRows.map(rowStr => {
      const cols: string[] = [];
      let cell = '';
      let d = 0;
      for (let i = 0; i < rowStr.length; i++) {
        if (rowStr[i] === '{') d += 1;
        else if (rowStr[i] === '}') d = Math.max(0, d - 1);
        if (d === 0 && rowStr[i] === '&') {
          cols.push(cell.trim());
          cell = '';
          continue;
        }
        cell += rowStr[i];
      }
      cols.push(cell.trim());
      return cols.map(c => new LatexParser(c).parseSequence());
    });

    return { kind: 'env', envName, rows };
  }

  parseSequence(stopChar?: string): MathNode[] {
    const nodes: MathNode[] = [];
    while (!this.eof()) {
      this.skipWs();
      if (this.eof() || (stopChar && this.src[this.pos] === stopChar)) break;
      const atom = this.parseAtom();
      if (!atom) break;

      let sub: MathNode[] | undefined;
      let sup: MathNode[] | undefined;
      while (!this.eof()) {
        this.skipWs();
        if (this.src[this.pos] === '_' && !sub) {
          this.pos += 1;
          sub = this.readGroup();
        } else if (this.src[this.pos] === '^' && !sup) {
          this.pos += 1;
          sup = this.readGroup();
        } else {
          break;
        }
      }

      if (atom.kind === 'bigop') {
        nodes.push({ ...atom, ...(sub ? { sub } : {}), ...(sup ? { sup } : {}) });
      } else if (sub || sup) {
        nodes.push({ kind: 'script', base: atom, ...(sub ? { sub } : {}), ...(sup ? { sup } : {}) });
      } else {
        nodes.push(atom);
      }
    }
    return nodes;
  }

  private parseAtom(): MathNode | null {
    this.skipWs();
    if (this.eof()) return null;
    const ch = this.src[this.pos];

    if (ch === '{') {
      this.pos += 1;
      const inner = this.parseSequence('}');
      if (this.src[this.pos] === '}') this.pos += 1;
      if (inner.length === 1) return inner[0];
      return { kind: 'text', value: nodesToInlineString(inner) };
    }

    if (ch === '\\') {
      this.pos += 1;
      if (this.eof()) return null;
      const next = this.src[this.pos];
      if (!/[a-zA-Z]/.test(next)) {
        this.pos += 1;
        if (next === ',' || next === ';' || next === ':' || next === ' ') return { kind: 'text', value: ' ' };
        if (next === '!') return { kind: 'text', value: '' };
        if (next === '|') return { kind: 'op', value: '‖' };
        return { kind: 'text', value: next };
      }
      const start = this.pos;
      while (!this.eof() && /[a-zA-Z]/.test(this.src[this.pos])) this.pos += 1;
      const cmd = this.src.slice(start, this.pos);

      if (cmd === 'begin') {
        const envName = this.readRawBraceText();
        return this.parseEnvironment(envName);
      }
      if (cmd === 'frac' || cmd === 'dfrac' || cmd === 'tfrac') {
        const num = this.readGroup();
        const den = this.readGroup();
        return { kind: 'frac', num, den };
      }
      if (cmd === 'binom' || cmd === 'dbinom' || cmd === 'tbinom') {
        const num = this.readGroup();
        const den = this.readGroup();
        return { kind: 'frac', num, den, noBar: true };
      }
      if (cmd === 'sqrt') {
        const index = this.readOptionalBracket();
        const body = this.readGroup();
        return { kind: 'sqrt', ...(index ? { index } : {}), body };
      }
      if (cmd === 'overline' || cmd === 'overrightarrow') {
        return { kind: 'overline', body: this.readGroup() };
      }
      if (cmd === 'underline') {
        return { kind: 'underline', body: this.readGroup() };
      }
      if (cmd === 'xrightarrow' || cmd === 'xleftarrow') {
        const below = this.readOptionalBracket();
        const above = this.readGroup();
        return { kind: 'xarrow', dir: cmd === 'xrightarrow' ? 'right' : 'left', above, ...(below ? { below } : {}) };
      }
      if (cmd === 'ce') {
        return { kind: 'text', value: formatChemistry(this.readRawBraceText()), italic: false };
      }
      if (cmd === 'mathbb') {
        const raw = this.readRawBraceText();
        const mapped = raw.split('').map(c => BLACKBOARD[c] ?? c).join('');
        return { kind: 'text', value: mapped, italic: false };
      }
      if (cmd === 'mathbf' || cmd === 'boldsymbol' || cmd === 'bm') {
        return { kind: 'text', value: nodesToInlineString(this.readGroup()), italic: false, bold: true };
      }
      if (cmd === 'text' || cmd === 'mathrm' || cmd === 'operatorname' || cmd === 'textrm') {
        return { kind: 'text', value: this.readRawBraceText(), italic: false };
      }
      if (cmd === 'mathcal' || cmd === 'mathscr') {
        return { kind: 'text', value: this.readRawBraceText(), italic: true };
      }
      if (cmd in ACCENTS) {
        const base = nodesToInlineString(this.readGroup());
        return { kind: 'text', value: base + ACCENTS[cmd], italic: true };
      }
      if (cmd === 'left' || cmd === 'right' || cmd === 'big' || cmd === 'Big' || cmd === 'bigg') {
        this.skipWs();
        if (!this.eof()) {
          if (this.src[this.pos] === '\\') {
            this.pos += 1;
            const d = this.src[this.pos++];
            return { kind: 'text', value: d === '|' ? '‖' : d };
          }
          const delim = this.src[this.pos++];
          return delim === '.' ? { kind: 'text', value: '' } : { kind: 'text', value: delim };
        }
        return { kind: 'text', value: '' };
      }
      if (BIG_OPS.has(cmd)) {
        return { kind: 'bigop', symbol: SYMBOLS[cmd] ?? cmd };
      }
      if (LIMIT_FUNCS.has(cmd)) {
        return { kind: 'bigop', symbol: cmd, isTextOp: true };
      }
      if (UPRIGHT_FUNCS.has(cmd)) {
        return { kind: 'text', value: `${cmd} `, italic: false };
      }
      if (cmd in SYMBOLS) {
        return { kind: 'op', value: SYMBOLS[cmd] };
      }
      return { kind: 'text', value: cmd };
    }

    if ('+-=<>(),[]|/:!'.includes(ch)) {
      this.pos += 1;
      return { kind: 'op', value: ch === '-' ? '−' : ch };
    }

    if (/[0-9.]/.test(ch)) {
      const start = this.pos;
      while (!this.eof() && /[0-9.]/.test(this.src[this.pos])) this.pos += 1;
      return { kind: 'text', value: this.src.slice(start, this.pos), italic: false };
    }

    this.pos += 1;
    return { kind: 'text', value: ch, italic: /[a-zA-Z]/.test(ch) };
  }
}

// TeX's atom classes for spacing: a relation takes a thick space each side, a binary operator a medium one.
const RELATIONS = new Set('=<>≤≥≠≈≡≅∼≃∝≪≫→←↔⇒⇐⇔↦↑↓⇌∈∉∋⊂⊃⊆⊇⊥∥:'.split(''));
const BINARIES = new Set('+−±∓×÷·∗⋆∘•⊕⊗⊖∪∩∖∧∨'.split(''));
type Spacing = 'rel' | 'bin' | null;

/** The space around each atom of a row: a binary with nothing to its left, or after a relation or operator, is a sign. */
function spacingOf(nodes: readonly MathNode[]): Spacing[] {
  const out: Spacing[] = [];
  nodes.forEach((n, i) => {
    const value = n.kind === 'op' ? n.value : '';
    if (RELATIONS.has(value)) out.push('rel');
    else if (BINARIES.has(value)) {
      const prev = i > 0 ? nodes[i - 1] : undefined;
      const isSign = prev === undefined || out[i - 1] !== null || (prev.kind === 'op' && '(,[|'.includes(prev.value)) || i === nodes.length - 1;
      out.push(isSign ? null : 'bin');
    } else out.push(null);
  });
  return out;
}

function nodesToInlineString(nodes: readonly MathNode[]): string {
  let out = '';
  const spacing = spacingOf(nodes);
  for (const [i, n] of nodes.entries()) {
    switch (n.kind) {
      case 'text':
        out += n.value;
        break;
      case 'op':
        out += spacing[i] ? ` ${n.value} ` : n.value;
        break;
      case 'frac':
        out += n.noBar
          ? `C(${nodesToInlineString(n.num)}, ${nodesToInlineString(n.den)})`
          : `(${nodesToInlineString(n.num)})/(${nodesToInlineString(n.den)})`;
        break;
      case 'sqrt': {
        const idx = n.index ? (mapChars(nodesToInlineString(n.index), SUPER_MAP) ?? nodesToInlineString(n.index)) : '';
        out += `${idx}√(${nodesToInlineString(n.body)})`;
        break;
      }
      case 'overline':
      case 'underline':
        out += nodesToInlineString(n.body);
        break;
      case 'xarrow':
        out += ` ${n.dir === 'right' ? '→' : '←'} `;
        break;
      case 'bigop': {
        const sub = n.sub ? nodesToInlineString(n.sub) : '';
        const sup = n.sup ? nodesToInlineString(n.sup) : '';
        const subM = sub ? (mapChars(sub, SUB_MAP) ?? `_${sub}`) : '';
        const supM = sup ? (mapChars(sup, SUPER_MAP) ?? `^${sup}`) : '';
        out += `${n.symbol}${subM}${supM} `;
        break;
      }
      case 'script': {
        const base = nodesToInlineString([n.base]);
        const sub = n.sub ? nodesToInlineString(n.sub) : '';
        const sup = n.sup ? nodesToInlineString(n.sup) : '';
        const subM = sub ? (mapChars(sub, SUB_MAP) ?? `_${sub}`) : '';
        const supM = sup ? (mapChars(sup, SUPER_MAP) ?? `^${sup}`) : '';
        out += `${base}${subM}${supM}`;
        break;
      }
      case 'env':
        out += n.rows.map(r => r.map(nodesToInlineString).join(', ')).join('; ');
        break;
    }
  }
  return out.replace(/\s{2,}/g, ' ').trim();
}

/** Converts inline LaTeX (e.g. `E = mc^2`, `\ce{H2SO4}`, `\mathbb{R}^n`) into clean Unicode math text. */
export function formatInlineLatex(latex: string): string {
  const nodes = new LatexParser(latex).parseSequence();
  return nodesToInlineString(nodes) || latex;
}

function envDelimiters(envName: string): { left: string | null; right: string | null } {
  switch (envName) {
    case 'pmatrix': return { left: '(', right: ')' };
    case 'bmatrix': return { left: '[', right: ']' };
    case 'Bmatrix': return { left: '{', right: '}' };
    case 'vmatrix': return { left: '|', right: '|' };
    case 'Vmatrix': return { left: '‖', right: '‖' };
    case 'cases': return { left: '{', right: null };
    default: return { left: null, right: null };
  }
}

function BracketEdge({ kind }: { readonly kind: '(' | ')' | '[' | ']' | '{' | '}' | '|' | '‖' }) {
  if (kind === '[') return <view aria-hidden className={m.bracket.squareOpen} />;
  if (kind === ']') return <view aria-hidden className={m.bracket.squareClose} />;
  if (kind === '|') return <view aria-hidden className={m.bracket.bar} />;
  if (kind === '‖') {
    return (
      <view aria-hidden className={m.bracket.doubleBar}>
        <view className={m.bracket.barLine} />
        <view className={m.bracket.barLine} />
      </view>
    );
  }
  if (kind === '(') return <view aria-hidden className={m.bracket.roundOpen} />;
  if (kind === ')') return <view aria-hidden className={m.bracket.roundClose} />;
  return <text aria-hidden className={m.bracket.glyph}>{kind}</text>;
}

// `compact`: a fraction's or a script's smaller type; `tight`: script style, where TeX sets relations and operators without space.
function renderMathNodes(nodes: readonly MathNode[], keyPrefix: string, compact = false, tight = false): Child[] {
  const textCls = compact ? m.text.compact : m.text.base;
  const scriptCls = m.script.text;
  const spacing = tight ? [] : spacingOf(nodes);

  return nodes.map((node, idx) => {
    const key = `${keyPrefix}-${idx}`;
    switch (node.kind) {
      case 'text':
        return (
          <text key={key} className={cx(textCls, node.italic && m.text.italic, node.bold && m.text.bold)}>
            {node.value}
          </text>
        );
      case 'op': {
        const space = spacing[idx];
        // A face's U+2212 can be narrower than its plus; it takes the plus's width so a sum stays even.
        if (node.value === '−') {
          return (
            <view key={key} className={cx(m.minus.base, space && m.space[space][compact ? 'compact' : 'base'])}>
              <text aria-hidden className={cx(textCls, m.minus.ghost)}>+</text>
              <text className={cx(textCls, m.minus.glyph)}>−</text>
            </view>
          );
        }
        return (
          <text key={key} className={cx(textCls, space && m.space[space][compact ? 'compact' : 'base'])}>
            {node.value}
          </text>
        );
      }
      case 'frac': {
        const inner = (
          <view className={m.frac.base}>
            <view className={m.frac.part}>
              {renderMathNodes(node.num, `${key}-n`, true)}
            </view>
            {!node.noBar && <view aria-hidden className={m.frac.bar} />}
            <view className={m.frac.part}>
              {renderMathNodes(node.den, `${key}-d`, true)}
            </view>
          </view>
        );
        if (node.noBar) {
          return (
            <view key={key} className={m.frac.binomial}>
              <BracketEdge kind="(" />
              {inner}
              <BracketEdge kind=")" />
            </view>
          );
        }
        return <view key={key}>{inner}</view>;
      }
      case 'sqrt':
        return (
          <view key={key} className={m.sqrt.base}>
            {node.index && (
              <view className={m.sqrt.index}>
                <text className={scriptCls}>{nodesToInlineString(node.index)}</text>
              </view>
            )}
            <text className={cx(textCls, m.sqrt.sign)}>√</text>
            <view className={m.sqrt.body}>
              {renderMathNodes(node.body, `${key}-sq`, compact, tight)}
            </view>
          </view>
        );
      case 'overline':
        return (
          <view key={key} className={m.overline.base}>
            {renderMathNodes(node.body, `${key}-ov`, compact, tight)}
          </view>
        );
      case 'underline':
        return (
          <view key={key} className={m.underline.base}>
            {renderMathNodes(node.body, `${key}-un`, compact, tight)}
          </view>
        );
      case 'xarrow':
        return (
          <view key={key} className={m.xarrow.base}>
            <view className={m.row.baseline}>
              {renderMathNodes(node.above, `${key}-xa`, true, true)}
            </view>
            <text className={textCls}>{node.dir === 'right' ? '───→' : '←───'}</text>
            {node.below && (
              <view className={m.row.baseline}>
                {renderMathNodes(node.below, `${key}-xb`, true, true)}
              </view>
            )}
          </view>
        );
      case 'bigop':
        if (node.isTextOp) {
          return (
            <view key={key} className={m.bigop.textOp}>
              <text className={textCls}>{node.symbol}</text>
              {node.sub && (
                <view className={m.row.baseline}>
                  {renderMathNodes(node.sub, `${key}-lsub`, true, true)}
                </view>
              )}
            </view>
          );
        }
        return (
          <view key={key} className={m.bigop.base}>
            <text className={m.bigop.symbol}>{node.symbol}</text>
            {(node.sup || node.sub) && (
              <view className={m.bigop.limits}>
                <view className={m.row.baseline}>
                  {node.sup ? renderMathNodes(node.sup, `${key}-sup`, true, true) : <text className={scriptCls}> </text>}
                </view>
                <view className={m.row.baseline}>
                  {node.sub ? renderMathNodes(node.sub, `${key}-sub`, true, true) : <text className={scriptCls}> </text>}
                </view>
              </view>
            )}
          </view>
        );
      case 'script': {
        // Only treat simple non-nested runs as flat; nested scripts like `e^{-x^2}` render as uniform small text
        // without mixing Unicode `²` and baseline `-` in the same string!
        const toPlainScript = (ns: readonly MathNode[]): string =>
          ns
            .map(n => {
              if (n.kind === 'text' || n.kind === 'op') return n.value;
              if (n.kind === 'script') {
                const b = toPlainScript([n.base]);
                const sp = n.sup ? (mapChars(toPlainScript(n.sup), SUPER_MAP) ?? `^(${toPlainScript(n.sup)})`) : '';
                const sb = n.sub ? (mapChars(toPlainScript(n.sub), SUB_MAP) ?? `_(${toPlainScript(n.sub)})`) : '';
                return `${b}${sb}${sp}`;
              }
              return nodesToInlineString([n]);
            })
            .join('');
        const isFlat = (ns: readonly MathNode[]): boolean =>
          ns.every(
            n =>
              n.kind === 'text' ||
              n.kind === 'op' ||
              (n.kind === 'script' && isFlat([n.base]) && (!n.sup || isFlat(n.sup)) && (!n.sub || isFlat(n.sub))),
          );
        const scriptPart = (ns: readonly MathNode[], suffix: string) =>
          isFlat(ns) ? <text className={scriptCls}>{toPlainScript(ns)}</text> : renderMathNodes(ns, `${key}-${suffix}`, true, true);
        const rowAlign = node.sup && !node.sub ? m.script.raised : node.sub && !node.sup ? m.script.lowered : m.script.centred;
        return (
          <view key={key} className={cx(m.script.base, rowAlign)}>
            {renderMathNodes([node.base], `${key}-b`, compact, tight)}
            <view className={node.sup && node.sub ? m.script.both : undefined}>
              {node.sup && <view className={cx(m.row.baseline, !node.sub && m.script.supOnly)}>{scriptPart(node.sup, 'sup')}</view>}
              {node.sub && <view className={cx(m.row.baseline, !node.sup && m.script.subOnly)}>{scriptPart(node.sub, 'sub')}</view>}
            </view>
          </view>
        );
      }
      case 'env': {
        const { left, right } = envDelimiters(node.envName);
        const isCases = node.envName === 'cases';
        const isAligned = node.envName.startsWith('align') || node.envName === 'split';
        return (
          <view key={key} className={m.env.base}>
            {left && <BracketEdge kind={left as '(' | ')' | '[' | ']' | '{' | '}' | '|' | '‖'} />}
            <view className={m.env.rows}>
              {node.rows.map((row, rIdx) => (
                <view key={`${key}-r-${rIdx}`} className={cx(m.env.row, isCases || isAligned ? m.env.rowAligned : m.env.rowMatrix)}>
                  {row.map((cell, cIdx) => (
                    <view
                      key={`${key}-c-${rIdx}-${cIdx}`}
                      className={cx(m.env.cell, isCases || isAligned ? m.env.cellAligned : m.env.cellMatrix)}
                    >
                      {renderMathNodes(cell, `${key}-cell-${rIdx}-${cIdx}`, compact, tight)}
                    </view>
                  ))}
                </view>
              ))}
            </view>
            {right && <BracketEdge kind={right as '(' | ')' | '[' | ']' | '{' | '}' | '|' | '‖'} />}
          </view>
        );
      }
    }
  });
}

/**
 * Native GPU View renderer for display LaTeX formulas:
 * stacked fractions, binomials, nth roots, matrices (`pmatrix`/`bmatrix`/`vmatrix`),
 * piecewise `cases`, `aligned` equations, big operators & `\lim`, `\ce{...}` chemistry, and full symbol table.
 */
export function MathFormulaView({ latex }: { readonly latex: string }) {
  const nodes = new LatexParser(latex).parseSequence();
  return (
    <view className={m.root.base}>
      {nodes.length > 0 ? renderMathNodes(nodes, 'm') : <text className={m.text.base}>{latex}</text>}
    </view>
  );
}
