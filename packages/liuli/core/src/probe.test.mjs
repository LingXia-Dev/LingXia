import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { probe } from './testing/probe.mjs';

// Every example, and every overlay open, drawn with the theme's steps left as references: a pack moves all of it.
const playground = fileURLToPath(new URL('../../gallery/shared/playground.generated.tsx', import.meta.url));
const overlays = ['ActionSheet', 'BottomSheet', 'CommandPalette', 'Dialog', 'Drawer', 'DropdownMenu', 'Popover', 'Tooltip'];
const opened = `import { PlaygroundSubject, playgroundEntries } from ${JSON.stringify(playground)};
export function render() {
  return <page><scroll>{playgroundEntries.filter(entry => ${JSON.stringify(overlays)}.includes(entry.name)).map(entry =>
    <view key={entry.name} data-testid={'probe:' + entry.name + '/open'}><PlaygroundSubject name={entry.name} props={{ ...entry.defaults, isOpen: true }} set={() => {}} /></view>)}</scroll></page>;
}
`;
const found = await probe(fileURLToPath(new URL('..', import.meta.url)), { pages: [opened] });

/** Lengths an example passes in points, which no pack reaches by design. */
const appPoints = new Set([
  'Carousel/Rail: width=160',
  'ListDetail/Orders: width=360',
  'ListDetail/Workspace: width=200',
  'Split/DocumentSheetSplit: width=300',
  'Split/LiveStreamSplit: width=300',
]);

/** Lengths Liuli cannot state in a step yet: the info button centred on a label's line mixes a space step and a type leading. */
const mixedScales = new Set(['Field/Info: marginBottom=-7', 'Field/Info: marginTop=-7']);

test('every length Liuli draws is a theme step, so a pack reaches it', () => {
  assert.deepEqual(found.lengths.filter(line => !appPoints.has(line) && !mixedScales.has(line)), []);
});

/**
 * Edges the default theme leaves under 3:1 in high contrast: the input to the theme's high-contrast work, not a
 * Liuli fix. `accent-solid` dims there so white text on it reaches 7:1, and `line` meets 3:1 on the bare surfaces
 * only, so a tint under it takes it below.
 */
const themeGaps = new Set([
  'border accent-solid on canvas (high-contrast-dark)',
  'border accent-solid on surface (high-contrast-dark)',
  'border accent-solid on accent-soft over surface (high-contrast-dark)',
  'border line on text/6 over canvas (high-contrast-light)',
  'border line on accent-soft over canvas (high-contrast-dark)',
  'rule line on accent-soft over canvas (high-contrast-dark)',
]);

test('every colour drawn is a theme role', () => {
  assert.deepEqual(found.colours, []);
});

test('in high contrast every edge stands 3:1 off what it lies on, except the listed theme gaps', () => {
  const short = found.edges.filter(({ edge }) => !themeGaps.has(edge)).map(({ example, edge, ratio }) => `${example}: ${edge}: ${ratio}`);
  assert.deepEqual(short, []);
  const seen = new Set(found.edges.map(({ edge }) => edge));
  assert.deepEqual([...themeGaps].filter(edge => !seen.has(edge)), [], 'a theme gap closed: take it off the list');
});
