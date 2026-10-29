import postcss from 'postcss';
import { readFile, realpath } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';

// Expand imports in CSS source order; esbuild's parallel loader completion
// order must never decide which design token wins.
export async function loadStylesheet(path, chain = [], budget = { files: 0 }) {
  path = await realpath(resolve(path));
  if (chain.includes(path)) throw Error(`UI_CSS: cyclic import: ${[...chain, path].join(' -> ')}`);
  if (++budget.files > 128) throw Error('UI_CSS: stylesheet import limit exceeded');
  const root = postcss.parse(await readFile(path, 'utf8'), { from: path });
  // Face files are named relative to the stylesheet that declares them, which an import moves.
  root.walkAtRules('font-face', rule => rule.walkDecls('src', decl => {
    decl.value = decl.value.replace(/url\(\s*(['"]?)(\.\.?\/[^'")]+)\1\s*\)/, (_, quote, url) => `url("${resolve(dirname(path), url)}")`);
  }));
  classRulesAsUtilities(root);
  for (const node of [...root.nodes]) {
    if (node.type !== 'atrule' || node.name !== 'import') continue;
    const match = node.params.match(/^(['"])(\.\.?\/[^'"]+)\1$/);
    if (!match || node.nodes) throw node.error('UI_CSS: use a quoted relative stylesheet import without conditions');
    const imported = await loadStylesheet(resolve(dirname(path), match[2]), [...chain, path], budget);
    node.replaceWith(imported.nodes);
  }
  return root;
}

// A plain `.name { … }` rule, top level or in `@layer components|utilities`, is that class as the
// theme's `@utility name`, so a Tailwind theme ports as written.
export function classRulesAsUtilities(root) {
  let changed = false;
  const convert = rule => {
    const name = rule.selector.trim().match(/^\.([a-z][a-z0-9-]*)$/)?.[1];
    if (!name) return;
    rule.replaceWith(postcss.atRule({ name: 'utility', params: name, nodes: rule.nodes, source: rule.source }));
    changed = true;
  };
  for (const node of [...root.nodes]) {
    if (node.type === 'rule') convert(node);
    if (node.type === 'atrule' && node.name === 'layer' && ['components', 'utilities'].includes(node.params.trim()) && node.nodes) {
      for (const child of [...node.nodes]) if (child.type === 'rule') convert(child);
      node.replaceWith(node.nodes);
      changed = true;
    }
  }
  return changed;
}

export function tokenSelectorTheme(selector) {
  if (selector === ':root' || selector === ':root, :host') return 'all';
  return selector.match(/^\[data-theme=(['"])(light|dark)\1\]$/)?.[2];
}

const faceProps = new Set(['font-family', 'src', 'font-weight', 'font-style', 'font-display']);

/** `@font-face` blocks of a theme stylesheet: `{family, url, weight, style, line}` per face. */
export function fontFaces(root) {
  const faces = [];
  root.walkAtRules('font-face', rule => {
    const face = { line: rule.source?.start?.line };
    for (const decl of rule.nodes ?? []) {
      if (decl.type !== 'decl') continue;
      if (!faceProps.has(decl.prop)) throw decl.error(`UI_FONT: @font-face takes font-family, src, font-weight and font-style, not ${decl.prop}`);
      if (decl.prop === 'font-family') face.family = decl.value.trim().replace(/^['"]|['"]$/g, '');
      if (decl.prop === 'src') {
        const url = decl.value.match(/^url\(\s*['"]?([^'")]+)['"]?\s*\)(?:\s+format\([^)]*\))?$/);
        if (!url) throw decl.error('UI_FONT: @font-face src is one url(…) of a bundled .ttf, .otf or .ttc file');
        face.url = url[1];
      }
      if (decl.prop === 'font-weight') face.weight = decl.value.trim();
      if (decl.prop === 'font-style') face.style = decl.value.trim();
    }
    if (!face.family || !/^[A-Za-z0-9 _-]{1,64}$/.test(face.family)) throw rule.error('UI_FONT: @font-face needs a font-family of letters, digits, spaces, _ or -');
    if (!face.url || !/\.(ttf|otf|ttc)$/i.test(face.url)) throw rule.error('UI_FONT: @font-face src is one url(…) of a bundled .ttf, .otf or .ttc file');
    faces.push(face);
  });
  return faces;
}

export function validateTokens(root) {
  for (const node of root.nodes) {
    if (node.type === 'comment') continue;
    // Bundled faces are declared at the top level, as in CSS.
    if (node.type === 'atrule' && node.name === 'font-face') continue;
    // A theme-owned class (`.brand-gradient {…}` in CSS) is a Tailwind `@utility` of plain declarations.
    if (node.type === 'atrule' && node.name === 'utility') {
      if (!/^[a-z][a-z0-9-]*$/.test(node.params)) throw node.error(`UI_CSS: @utility ${node.params}: name a plain utility, such as brand-gradient`);
      for (const child of node.nodes ?? []) if (child.type !== 'decl' && child.type !== 'comment')
        throw child.error(`UI_CSS: @utility ${node.params} takes declarations only; put states on the element (focus:, active:)`);
      continue;
    }
    const isTheme = node.type === 'atrule' && node.name === 'theme';
    const palette = node.type === 'rule' && tokenSelectorTheme(node.selector);
    if (!isTheme && !palette) throw node.error('UI_CSS: expected @theme, :root or [data-theme] tokens, @utility, or a plain .class rule');
    for (const child of node.nodes ?? []) {
      if (child.type === 'comment') continue;
      if (isTheme && child.type === 'atrule' && child.name === 'keyframes') {
        if (!/^[-_a-zA-Z][-_a-zA-Z0-9]*$/.test(child.params) || !child.nodes) throw child.error('UI_ANIMATION: invalid keyframe name or body');
        continue;
      }
      const token = child.type === 'decl' && child.prop.startsWith('--');
      // View.env already owns color-scheme. Permit the equivalent annotation
      // so browser and native palettes can share the same token source.
      const scheme = palette && child.type === 'decl' && child.prop === 'color-scheme' && child.value === (palette === 'all' ? 'light' : palette);
      if ((!token && !scheme) || child.important) throw child.error('UI_CSS: only custom properties are supported in palette blocks');
    }
  }
}
