// The probe theme: a package's examples rendered with every theme step left as a reference to its scale, as a
// pack with a sentinel in every scale would draw them. A length that reaches the wire as a plain number is one
// no pack can move; an edge high contrast cannot lift to 3:1 fails the theme's high-contrast rule.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { palette, ratio, format } from '../../scripts/contrast.mjs';
import { harnessFor } from './harness.mjs';

const theme = await palette();
const isStep = value => Array.isArray(value) && value.length === 3 && value.every(Number.isFinite);
// Hairlines and pills are no scale's step: a 1 pt rule, a visually hidden box, and the full radius.
const unscaled = new Set([0, 1, -1, 16384]);
const transparent = colour => /^#[0-9a-f]{6}00$/i.test(colour);
const quiet = new Set(['text/8', 'text/12']);

/** `[module, example]` for each example of the package at `dir`, a module per component folder. */
function examplesOf(dir) {
  const src = join(dir, 'src');
  return readdirSync(src, { withFileTypes: true }).filter(entry => entry.isDirectory()).flatMap(({ name }) => {
    let source;
    try { source = readFileSync(join(src, name, `${name}.examples.tsx`), 'utf8'); } catch { return []; }
    return [...source.matchAll(/^export function ([A-Z]\w*)/gm)].map(match => [name, match[1]]);
  });
}

function pageOf(examples) {
  const modules = [...new Set(examples.map(([module]) => module))];
  return linked => modules.map(module => `import * as ${module}Examples from ${JSON.stringify(join(linked, 'src', module, `${module}.examples`))};`).join('\n') +
    '\nexport function render() { return <page><scroll>' +
    examples.map(([module, example]) => `<view data-testid=${JSON.stringify(`probe:${module}/${example}`)}><${module}Examples.${example} /></view>`).join('') +
    '</scroll></page>; }\n';
}

const ownerOf = node => {
  for (let at = node; at; at = at.parent) if (at.testId?.startsWith('probe:')) return at.testId.slice(6);
  return '?';
};

const surfaceRoles = new Set(['canvas', 'surface', 'surface-sunken', 'surface-raised', 'surface-overlay']);
// A role the theme draws translucent in some look (`scrim`): a fill over a ground, not a ground.
const seeThrough = role => Object.values(theme.roles).some(roles => roles[role]?.length === 9 && !roles[role].endsWith('ff'));
const fillOf = node => {
  const background = node.props.background;
  if (typeof background !== 'string' || background.startsWith('#')) return undefined;
  return { colour: background, translucent: background.includes('/') || background.endsWith('-soft') || seeThrough(background), node };
};

/** What an edge of `node` lies on: the opaque ground from `from` up, with the translucent fill nearest it over that ground. */
function groundOf(from) {
  let fill;
  for (let at = from; at; at = at.parent) {
    const found = fillOf(at);
    if (!found) continue;
    if (found.translucent) { fill ??= found; continue; }
    return fill ? { pair: fill.colour, over: found.colour, by: fill.node } : { pair: found.colour, by: at };
  }
  return fill ? { pair: fill.colour, over: 'canvas', by: fill.node } : { pair: 'canvas' };
}

const measure = (roles, colour, ground) => ratio(roles, { pair: [colour, ground.pair], ...(ground.over ? { over: ground.over } : {}) });

/** Every edge `node` draws in its resting look: its border, its rings, or the whole box as a 1 pt rule. */
function edgesOf(node) {
  const { props } = node;
  const edges = [];
  const bordered = ['borderWidth', 'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth', 'borderInlineStartWidth', 'borderInlineEndWidth'].some(key => props[key] > 0);
  if (bordered && typeof props.borderColor === 'string') edges.push({ what: 'border', colour: props.borderColor });
  if (typeof props.ring === 'string' && props.ring.startsWith('{')) {
    const ring = JSON.parse(props.ring);
    if (ring.inner?.width > 0) edges.push({ what: 'inset ring', colour: ring.inner.color });
    // A translucent ring around a solid border is a halo; the border is the edge.
    if (ring.outer?.width > 0 && !(bordered && ring.outer.color.includes('/'))) edges.push({ what: 'ring', colour: ring.outer.color });
  }
  if ((props.width === 1 || props.height === 1) && typeof props.background === 'string') edges.push({ what: 'rule', colour: props.background });
  return edges.filter(({ colour }) => !colour.startsWith('#') || !transparent(colour));
}

const hidden = node => {
  for (let at = node; at; at = at.parent) if (at.props.opacity === 0) return true;
  return false;
};

/**
 * Renders every example of the package at `dir` (with `pages`, more page sources of the package) at a
 * phone and a desktop viewport, each again a few points larger so a length taken from the viewport moves.
 * Returns the literal lengths and colours as `module/example: field=value`, and the high-contrast edges under
 * 3:1, each `{ example, edge, ratio }`.
 */
export async function probe(dir, { pages = [], group = 6 } = {}) {
  const { renderPage } = harnessFor(dir);
  const examples = examplesOf(dir);
  const sources = [];
  for (let at = 0; at < examples.length; at += group) sources.push(pageOf(examples.slice(at, at + group)));
  sources.push(...pages);
  const scaled = new Set();
  const lengths = new Map();
  const colours = new Set();
  const edges = [];

  const read = screen => {
    const found = [];
    const walk = (props, node) => {
      for (const [key, value] of Object.entries(props)) {
        if (isStep(value)) scaled.add(key);
        else if (typeof value === 'number' && !unscaled.has(value)) found.push([key, `${ownerOf(node)}: ${key}=${Math.round(value * 100) / 100}`]);
        // A colour is a role; a literal one, other than nothing, no theme reaches. A video's letterbox is the runtime's black.
        else if (node.tag !== 'video' && typeof value === 'string' && /^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(value) && !transparent(value)) colours.add(`${ownerOf(node)}: ${key}=${value}`);
      }
    };
    for (const node of screen.nodes) walk(node.props, node);
    return found;
  };

  const shortEdges = screen => {
    for (const node of screen.nodes) {
      if (hidden(node)) continue;
      const own = fillOf(node);
      for (const { what, colour } of edgesOf(node)) {
        const role = colour.split('/')[0];
        // Not an edge to measure: the quiet edge only a disabled look may draw, a tone's edge on its own soft fill,
        // media chrome, and a cutout drawn in a surface's colour to part overlapping shapes.
        if (quiet.has(colour) || role === 'on-media' || own?.colour === `${role}-soft` || surfaceRoles.has(role)) continue;
        // An inset ring lies on the box's own fill; a border and a rule on what is around the box.
        const ground = groundOf(what === 'inset ring' && own && !own.translucent ? node : node.parent);
        // Inside a disabled look, whose box keeps the quiet edge; or an overlay's panel, which its scrim sets apart.
        if (quiet.has(ground.by?.props.borderColor) || ground.pair === 'scrim') continue;
        for (const look of ['high-contrast-light', 'high-contrast-dark']) {
          const roles = theme.roles[look];
          // A box whose own opaque fill stands 3:1 off its ground shows its bounds without the border.
          if (what === 'border' && own && !own.translucent && measure(roles, own.colour, ground).value >= 3) continue;
          const result = measure(roles, colour, ground);
          if (!result.missing && result.value < 3) edges.push({ example: ownerOf(node), edge: `${what} ${colour} on ${ground.over ? `${ground.pair} over ${ground.over}` : ground.pair} (${look})`, ratio: format(result.value) });
        }
      }
    }
  };

  const viewports = [[{ width: 400, height: 900 }, { width: 413, height: 917 }], [{ width: 1100, height: 900 }, { width: 1113, height: 917 }]];
  await Promise.all(sources.map(async page => {
    const screen = await renderPage(page, { env: { viewport: viewports[0][0] } }, { points: false });
    for (const [first, second] of viewports) {
      screen.rerender({ env: { viewport: first } });
      const found = read(screen);
      shortEdges(screen);
      screen.rerender({ env: { viewport: second } });
      const kept = new Set(read(screen).map(([, line]) => line));
      for (const [key, line] of found) if (kept.has(line)) lengths.set(line, key);
    }
  }));
  return {
    lengths: [...lengths].filter(([, key]) => scaled.has(key)).map(([line]) => line).sort(),
    colours: [...colours].sort(),
    edges: [...new Map(edges.map(found => [`${found.example}: ${found.edge}`, found])).values()],
  };
}
