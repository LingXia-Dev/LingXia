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
  for (const node of [...root.nodes]) {
    if (node.type !== 'atrule' || node.name !== 'import') continue;
    const match = node.params.match(/^(['"])(\.\.?\/[^'"]+)\1$/);
    if (!match || node.nodes) throw node.error('UI_CSS: use a quoted relative stylesheet import without conditions');
    const imported = await loadStylesheet(resolve(dirname(path), match[2]), [...chain, path], budget);
    node.replaceWith(imported.nodes);
  }
  return root;
}

export function tokenSelectorTheme(selector) {
  if (selector === ':root' || selector === ':root, :host') return 'all';
  return selector.match(/^\[data-theme=(['"])(light|dark)\1\]$/)?.[2];
}

export function validateTokens(root, { animationLowering = false } = {}) {
  for (const node of root.nodes) {
    if (node.type === 'comment') continue;
    const isTheme = node.type === 'atrule' && node.name === 'theme';
    const palette = node.type === 'rule' && tokenSelectorTheme(node.selector);
    if (!isTheme && !palette) throw node.error('UI_CSS: expected @theme, :root or [data-theme] token declarations');
    for (const child of node.nodes ?? []) {
      if (child.type === 'comment') continue;
      if (isTheme && child.type === 'atrule' && child.name === 'keyframes') {
        if (!animationLowering) throw child.error('UI_ANIMATION: native presentation scheduling is not connected');
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
