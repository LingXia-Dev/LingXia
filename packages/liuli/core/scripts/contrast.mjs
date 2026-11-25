// WCAG contrast for the doc files' role pairs, computed from the default theme's role values.
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const appearances = ['light', 'dark'];
/** Every look the default theme resolves, high contrast included. */
export const looks = ['light', 'dark', 'high-contrast-light', 'high-contrast-dark'];
/** The grounds a line can sit on. */
export const surfaces = ['canvas', 'surface', 'surface-sunken', 'surface-raised', 'surface-overlay'];

// What a role draws as under a surface, as native paints it: `line` inside an overlay is the overlay's line.
const theme = await import('@lingxia/view/theme').catch(() => undefined);
export const roleUnder = (role, surface) => theme?.roleUnder ? theme.roleUnder(role, surface) : role;

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..');
const pack = resolve(repo, 'crates/lingxia-theme/packs/default.json');

/** Every colour role the View contract defines. */
export const roleNames = JSON.parse(readFileSync(resolve(repo, 'contracts/view/schema.json'), 'utf8')).roles.map(role => role.name);

/**
 * Role values per appearance as `#rrggbb[aa]`. The generated resolved palette wins when
 * @lingxia/view ships one; until then the pack JSON, which lacks the roles the engine derives
 * from the accent seed (accent family, focus, selection).
 */
export async function palette() {
  const generated = await import('@lingxia/view/theme').then(m => m.defaultPalette, () => undefined);
  if (generated) return { source: '@lingxia/view/theme', roles: generated };
  return { source: 'crates/lingxia-theme/packs/default.json', roles: JSON.parse(readFileSync(pack, 'utf8')).appearance };
}

const rgba = hex => {
  const h = hex.replace('#', '');
  const n = i => parseInt(h.slice(i, i + 2), 16) / 255;
  return [n(0), n(2), n(4), h.length === 8 ? n(6) : 1];
};

const over = ([r, g, b, a], [br, bg, bb]) => [r * a + br * (1 - a), g * a + bg * (1 - a), b * a + bb * (1 - a), 1];

const luminance = rgb => {
  const [r, g, b] = rgb.slice(0, 3).map(c => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** `role` or `role/NN` (NN percent opacity); `undefined` when the palette lacks the role. */
function layer(roles, spec) {
  const [role, percent] = spec.split('/');
  if (roles[role] === undefined) return undefined;
  const color = rgba(roles[role]);
  if (percent !== undefined) color[3] *= Number(percent) / 100;
  return color;
}

/**
 * The ratio of `pair.pair[0]` on `pair.pair[1]`, the background painted over `pair.over` when
 * translucent; `{ missing }` names the roles the palette lacks.
 */
export function ratio(roles, pair) {
  // The foreground resolves under the surface it sits on: the translucent background's backdrop, or the background.
  const [role, percent] = pair.pair[0].split('/');
  const scope = (pair.over ?? pair.pair[1]).split('/')[0];
  const drawn = [roleUnder(role, scope), percent].filter(part => part !== undefined).join('/');
  const specs = [drawn, pair.pair[1], ...(pair.over ? [pair.over] : [])];
  const missing = specs.map(spec => spec.split('/')[0]).filter(role => roles[role] === undefined);
  if (missing.length) return { missing: [...new Set(missing)] };
  const [fg, bg, base] = specs.map(spec => layer(roles, spec));
  const ground = base ? over(bg, over(base, [1, 1, 1])) : over(bg, [1, 1, 1]);
  const [a, b] = [luminance(over(fg, ground)), luminance(ground)];
  return { value: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05) };
}

export const needed = pair => (pair.minimum === '3:1' ? 3 : 4.5);

/** Two decimals, rounded down, so a printed ratio never claims more than it is. */
export const format = value => `${(Math.floor(value * 100) / 100).toFixed(2)}:1`;
