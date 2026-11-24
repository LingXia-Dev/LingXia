import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { renderExample, renderPage } from '../testing/harness.mjs';

const drawing = node => JSON.parse(node.props.vectorSource);
const source = JSON.parse(readFileSync(new URL('../../icons/icons.json', import.meta.url), 'utf8'));
const names = Object.keys(source.icons);

test('Common: an icon is a 20 pt drawing on a 24 grid, stroked in the current colour, hidden from assistive tech', async () => {
  const screen = await renderExample('Icon/Icon', 'Common');
  const icon = screen.getByTestId('search');
  assert.equal(icon.tag, 'vector');
  assert.deepEqual([icon.props.width, icon.props.height, icon.props.shrink, icon.props.accessibilityHidden], [20, 20, 0, 1]);
  const { viewBox, paths } = drawing(icon);
  assert.deepEqual(viewBox, [0, 0, 24, 24]);
  // One path for the handle and the glass, so an ink with alpha is laid once where they meet.
  assert.equal(paths.length, 1);
  assert.ok(paths.every(path => path.fill === 'none' && path.stroke === 'currentColor' && path.strokeWidth === 1.8 && path.strokeLinecap === 'round'));
  assert.match(paths[0].d, /M3 11a8 8 0 1 0 16 0a8 8 0 1 0 -16 0z$/, 'a circle is drawn as two arcs');
});

test('Sizes: 16, 20 and 24', async () => {
  const screen = await renderExample('Icon/Icon', 'Sizes');
  assert.deepEqual(['small', 'medium', 'large'].map(id => screen.getByTestId(id).props.width), [16, 20, 24]);
});

test('Inline: an inline icon is a square in em of the text around it, 20 pt beside body text', async () => {
  const screen = await renderExample('Icon/Icon', 'Inline');
  for (const id of ['in-title', 'in-footnote']) {
    const { width, height } = screen.getByTestId(id).props;
    assert.ok(Array.isArray(width) && String(width) === String(height), `${id}: a square in em`);
    assert.equal(Math.round(width[1] * 15), 20, `${id}: 1.33 em, 20 pt at the body's 15`);
  }
});

test('Labelled: with a label the icon is an image named by it; without one it is not read', async () => {
  const screen = await renderExample('Icon/Icon', 'Labelled');
  const named = screen.getByTestId('private');
  assert.deepEqual([named.tag === 'vector', named.props.role, named.props.accessibilityLabel, named.props.width], [false, 'img', 'Private', 20]);
  assert.equal(named.children[0].props.accessibilityHidden, 1);
  assert.equal(named.props.color, 'text-muted', 'a colour class lands on the box and the drawing inherits it');
  const silent = screen.getByTestId('silent');
  assert.deepEqual([silent.tag, silent.props.accessibilityHidden, silent.props.role], ['vector', 1, undefined]);
});

test('Mirrored: a directional icon carries the right-to-left flip; a search glass does not', async () => {
  const screen = await renderExample('Icon/Icon', 'Mirrored');
  const flipped = screen.getByTestId('forward-rtl');
  assert.equal(JSON.stringify(flipped.props).includes('scale'), true, 'in a right-to-left container the chevron is flipped');
  const ltr = await renderExample('Icon/Icon', 'Common');
  assert.equal(JSON.stringify(ltr.getByTestId('search').props).includes('scale'), false);
  for (const [name, entry] of Object.entries(source.icons))
    if (/(^|-)(start|end)$|^(send|undo|redo|sign-in|sign-out|external-link|list)$/.test(name)) assert.equal(entry.isMirrored, true, `${name} points along the reading direction`);
});

test('Custom: an own definition is drawn as given; a provided set redraws the names it covers and keeps the rest', async () => {
  const screen = await renderExample('Icon/Icon', 'Custom');
  const d = id => drawing(screen.getByTestId(id)).paths.map(path => path.d);
  assert.deepEqual(d('own'), ['M13 2 4.5 13.5H11L10 22l8.5-11.5H12z']);
  assert.equal(d('swapped-close').length, 2);
  assert.notDeepEqual(d('swapped-close'), d('default-close'));
  const plain = await renderExample('Icon/Icon', 'Common');
  assert.deepEqual(d('kept-search'), drawing(plain.getByTestId('search')).paths.map(path => path.d), 'a name the set leaves out comes from the default set');
});

test('Everything: every name draws, none blank', async () => {
  const screen = await renderExample('Icon/Icon', 'Everything');
  const icons = screen.queryAll(node => node.tag === 'vector');
  assert.equal(icons.length, names.length);
  assert.ok(names.length >= 80);
  for (const icon of icons) {
    const { paths } = drawing(icon);
    assert.ok(paths.length > 0 && paths.every(path => path.d.length > 3));
  }
  assert.deepEqual(screen.queryAll(node => node.tag === 'text').map(node => node.text), names);
});

test('a filled dot keeps its fill, and a rounded rectangle becomes arcs', async () => {
  const screen = await renderPage(`
    import { Icon } from '@liulidesign/core/Icon';
    export function render() { return <page><Icon name="tag" data-testid="tag" /><Icon name="calendar" data-testid="calendar" /></page>; }
  `);
  const dot = drawing(screen.getByTestId('tag')).paths[1];
  assert.deepEqual([dot.fill, dot.stroke], ['currentColor', undefined]);
  assert.ok(drawing(screen.getByTestId('calendar')).paths.some(path => path.d.includes('M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-14a2 2 0 0 1 2 -2z')));
});

test('the vendored set: one file per name in use, with its licence beside it', () => {
  const files = readdirSync(new URL('../../icons/lucide/', import.meta.url));
  assert.ok(files.includes('LICENSE'));
  assert.match(readFileSync(new URL('../../icons/lucide/LICENSE', import.meta.url), 'utf8'), /ISC License[\s\S]*Lucide Icons and Contributors/);
  assert.deepEqual(files.filter(file => file.endsWith('.svg')).sort(), [...new Set(Object.values(source.icons).map(entry => `${entry.lucide}.svg`))].sort());
  assert.deepEqual(readdirSync(new URL('./icons/', import.meta.url)).sort(), names.map(name => `${name}.ts`).sort());
});

test('a component draws its own marks from the set in use: a provided set redraws the status mark and close of a banner and the icon of a button', async () => {
  const screen = await renderPage(`
    import { Banner } from '@liulidesign/core/Banner';
    import { Button } from '@liulidesign/core/Button';
    import { IconSetProvider } from '@liulidesign/core/Icon';
    const cross = { paths: ['M7 7l10 10M17 7 7 17'] };
    const dot = { paths: ['M12 11v2'] };
    export function render() {
      return (
        <page>
          <Banner tone="warning" title="Offline" onDismiss={() => {}} dismissLabel="Dismiss" data-testid="plain" />
          <IconSetProvider icons={{ close: cross, warning: dot, add: dot }}>
            <view>
              <Banner tone="warning" title="Offline" onDismiss={() => {}} dismissLabel="Dismiss" data-testid="themed" />
              <Button label="New" startIcon="add" data-testid="button" />
            </view>
          </IconSetProvider>
        </page>
      );
    }
  `);
  const marks = id => {
    const found = [];
    const walk = node => { if (node.tag === 'vector') found.push(drawing(node).paths.map(path => path.d).join(' ')); node.children.forEach(walk); };
    walk(screen.getByTestId(id));
    return found;
  };
  assert.deepEqual(marks('themed'), ['M12 11v2', 'M7 7l10 10M17 7 7 17']);
  assert.notDeepEqual(marks('plain'), marks('themed'));
  assert.deepEqual(marks('button'), ['M12 11v2']);
});

test('an icon prop takes a definition as Icon does, drawn as given whatever set is in use', async () => {
  const screen = await renderPage(`
    import { Button } from '@liulidesign/core/Button';
    import { IconButton } from '@liulidesign/core/IconButton';
    import { TextInput } from '@liulidesign/core/TextInput';
    import { IconSetProvider } from '@liulidesign/core/Icon';
    const brand = { paths: ['M4 4h16v16h-16z'] };
    const dot = { paths: ['M12 11v2'] };
    export function render() {
      return (
        <page>
          <IconSetProvider icons={{ add: dot }}>
            <view>
              <Button label="Open" startIcon={brand} endIcon="add" data-testid="button" />
              <IconButton label="Open" icon={brand} data-testid="icon-button" />
              <TextInput value="" onChange={() => {}} label="Site" startIcon={brand} data-testid="input" />
            </view>
          </IconSetProvider>
        </page>
      );
    }
  `);
  const marks = node => screen.queryAll(n => n.tag === 'vector' && JSON.stringify(n).length > 0).filter(n => { let at = n; while (at && at !== node) at = at.parent; return at === node; }).map(n => drawing(n).paths[0].d);
  assert.deepEqual(marks(screen.getByTestId('button')), ['M4 4h16v16h-16z', 'M12 11v2']);
  assert.deepEqual(marks(screen.getByTestId('icon-button')), ['M4 4h16v16h-16z']);
  assert.deepEqual(marks(screen.getByTestId('input').parent), ['M4 4h16v16h-16z']);
});
