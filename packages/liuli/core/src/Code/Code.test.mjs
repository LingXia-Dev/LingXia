import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';
import { palette, ratio } from '../../scripts/contrast.mjs';

/** A text's runs as [text, colour], colour null for plain text. */
const runs = node => (node.children.length ? node.children : [node]).map(child => [child.text, child.tag === 'span' ? child.props.color ?? null : null]);
const textOf = node => runs(node).map(([text]) => text).join('');

const tick = () => new Promise(resolve => setImmediate(resolve));
const layer = screen => screen.queryAll(node => node.props.topLayer === 2)[0];

test('Command: a named group with its title, a copy button, and the code unwrapped in a sideways scroll', async () => {
  const screen = await renderExample('Code/Code', 'Command');
  const block = screen.getByTestId('install');
  assert.deepEqual([block.props.role, block.props.accessibilityLabel], ['group', 'Install commands']);
  const [header, scroll] = block.children;
  assert.equal(header.children[0].text, 'Terminal');
  const code = scroll.children[0];
  assert.deepEqual([scroll.tag, code.props.whiteSpace, code.text.split('\n').length], ['scroll', 'pre', 2]);
  assert.equal(screen.getByTestId('install-copy').props.accessibilityLabel, 'Copy');
});

test('onCopy: gets the whole code; then the button says Copied, a status bubble shows, and both go when the host closes it', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { CodeBlock } from '@liulidesign/core/Code';
    function Page() {
      const got = useLocal('got', '');
      return <view>
        <CodeBlock code={'a\\nb'} label="Sample" onCopy={code => got.set(code)} data-testid="sample" />
        <text data-testid="got">{got.value}</text>
      </view>;
    }
    export function render() { return <page><Page /></page>; }
  `);
  assert.equal(layer(screen), undefined, 'no bubble before a copy');
  screen.press(screen.getByTestId('sample-copy'));
  await tick();
  screen.rerender();
  assert.equal(screen.getByTestId('got').text, 'a\nb');
  assert.equal(screen.getByTestId('sample-copy').props.accessibilityLabel, 'Copied');
  const bubble = screen.getByTestId('sample-copied');
  assert.equal(bubble.props.role, 'status');
  assert.deepEqual([bubble.props.anchorArea, bubble.props.marginBottom], ['top', 4], 'above the button, off the code');
  assert.equal(layer(screen).props.closeAfter, 1500);
  screen.host.__lingxiaCloseAfter(bubble.props.elementId);
  screen.rerender();
  assert.equal(screen.getByTestId('sample-copy').props.accessibilityLabel, 'Copy');
});

test('onCopy: a rejected copy says nothing was copied', async () => {
  const screen = await renderPage(`
    import { CodeBlock } from '@liulidesign/core/Code';
    export function render() { return <page><CodeBlock code="x" label="Sample" onCopy={() => Promise.reject(new Error('denied'))} data-testid="sample" /></page>; }
  `);
  const errors = console.error;
  console.error = () => {};
  try {
    screen.press(screen.getByTestId('sample-copy'));
    await tick();
    screen.rerender();
  } finally {
    console.error = errors;
  }
  assert.equal(screen.getByTestId('sample-copy').props.accessibilityLabel, 'Copy');
  assert.equal(layer(screen), undefined);
});

test('Capped: the code scrolls up and down inside maxHeight, under the header', async () => {
  const block = (await renderExample('Code/Code', 'Capped')).getByTestId('capped');
  const [, capped] = block.children;
  assert.equal(capped.tag, 'scroll');
  assert.equal(capped.props.maxHeight, 160);
  assert.equal(capped.children[0].tag, 'scroll', 'the sideways scroll sits inside');
});

test('Wrapped: no sideways scroll; the code wraps at the width and breaks a long word', async () => {
  const block = (await renderExample('Code/Code', 'Wrapped')).getByTestId('wrapped');
  const [, wrap] = block.children;
  assert.equal(wrap.tag, 'column');
  const code = wrap.children[0];
  assert.deepEqual([code.props.whiteSpace, code.props.overflowWrap], ['pre-wrap', 'anywhere']);
});

test('Plain: no header without a title or an action', async () => {
  const block = (await renderExample('Code/Code', 'Plain')).getByTestId('response');
  assert.deepEqual(block.children.map(node => node.tag), ['scroll']);
});

test('InSentence: code is a run of its sentence, on a tint', async () => {
  const run = (await renderExample('Code/Code', 'InSentence')).getByTestId('key');
  assert.deepEqual([run.tag, run.props.background], ['span', 'text/8']);
});

test('Numbered: numbers from the start line beside one selectable text, marked lines tinted behind it', async () => {
  const block = (await renderExample('Code/Code', 'Numbered')).getByTestId('numbered');
  const lines = block.children[1].children[0];
  const [tints, gutter, code] = lines.children;
  assert.equal(gutter.props.accessibilityHidden, 1, 'numbers are not read');
  assert.equal(textOf(gutter), '41\n42\n43\n44\n45', 'one run while the lines are alike');
  assert.equal(textOf(code), 'export function total(items: readonly Item[]) {\n  let sum = 0;\n  for (const item of items) sum += item.price * item.count;\n  return Math.round(sum * 100) / 100;\n}');
  assert.equal(tints.children.length, 5);
  assert.deepEqual(tints.children.map(row => row.props.background ?? null), [null, null, 'accent-soft', 'accent-soft', null]);
});

test('Diff: signs in their own column, the code without them, headers quiet', async () => {
  const block = (await renderExample('Code/Code', 'Diff')).getByTestId('diff');
  const [tints, signs, code] = block.children[1].children[0].children;
  assert.equal(textOf(signs).replaceAll('\n', ''), '     -+ ');
  const shown = textOf(code).split('\n');
  assert.deepEqual([shown[0], shown[2], shown[3], shown[6]], ['--- a/cart.ts', '@@ -42,4 +42,4 @@', '  let sum = 0;', '    sum += item.price * item.count;'], 'headers whole, code lines without their sign');
  assert.deepEqual(runs(code).slice(0, 1), [['--- a/cart.ts\n+++ b/cart.ts\n@@ -42,4 +42,4 @@\n', 'text-muted']], 'headers quiet, not read as code');
  assert.ok(runs(code).some(([text, color]) => text === 'let' && color === 'accent'), 'the code is coloured by its language');
  assert.deepEqual(tints.children.map(row => row.props.background ?? null), ['text/6', 'text/6', 'text/6', null, null, 'danger-soft', 'success-soft', null]);
});

test('diffKinds: a file header only before a hunk; a lone "---" line is a removal', async () => {
  const { diffKinds } = await import('./diff.ts');
  assert.deepEqual(diffKinds(['--- a', '+++ b', '@@ -1 +1 @@', '-x', '+y', ' z']), ['header', 'header', 'header', 'removed', 'added', 'context']);
  assert.deepEqual(diffKinds(['--- not a header', 'y']), ['removed', 'context']);
});

test('Numbered: keywords, numbers and types take their colours; names and punctuation stay the text colour', async () => {
  const block = (await renderExample('Code/Code', 'Numbered')).getByTestId('numbered');
  const code = block.children[1].children[0].children[2];
  const colored = runs(code).filter(([, color]) => color !== null);
  assert.deepEqual(colored.slice(0, 6), [['export', 'accent'], ['function', 'accent'], ['readonly', 'accent'], ['Item', 'chart-7'], ['let', 'accent'], ['0', 'chart-4']]);
  assert.ok(runs(code).some(([text, color]) => text.includes('total(items: ') && color === null));
});

test('an unknown language, or none, draws the code plain in one run', async () => {
  for (const language of ['klingon', undefined]) {
    const screen = await renderPage(`
      import { CodeBlock } from '@liulidesign/core/Code';
      export function render() { return <page><CodeBlock code={'const a = "x";\\nreturn a;'} ${language ? `language="${language}"` : ''} label="Sample" data-testid="block" /></page>; }
    `);
    const text = screen.getByTestId('block').children[0].children[0];
    assert.deepEqual(runs(text), [['const a = "x";\nreturn a;', null]], String(language));
  }
});

test('code past about 800 coloured runs is several texts, stacked, together the whole code', async () => {
  const code = Array.from({ length: 400 }, (_, index) => `const v${index} = ${index}; // n`).join('\n');
  const screen = await renderPage(`
    import { CodeBlock } from '@liulidesign/core/Code';
    const code = ${JSON.stringify(code)};
    export function render() { return <page><CodeBlock code={code} language="ts" hasLineNumbers label="Long" data-testid="block" /></page>; }
  `);
  const column = screen.getByTestId('block').children[0].children[0].children[2];
  assert.equal(column.tag, 'column');
  const spans = column.children.map(text => text.children.length);
  assert.ok(spans.length > 1 && column.children.every(text => text.tag === 'text') && spans.every(count => count <= 800), `spans per text: ${spans}`);
  assert.ok(column.children.every(text => !textOf(text).endsWith('\n')), 'no text ends on a blank line');
  assert.equal(column.children.map(textOf).join('\n'), code);
});

test('a block that fits is one text: 120 coloured lines, under 800 spans', async () => {
  const code = Array.from({ length: 120 }, (_, index) => `const v${index} = ${index}; // n`).join('\n');
  const screen = await renderPage(`
    import { CodeBlock } from '@liulidesign/core/Code';
    const code = ${JSON.stringify(code)};
    export function render() { return <page><CodeBlock code={code} language="ts" hasLineNumbers label="Fits" data-testid="block" /></page>; }
  `);
  const text = screen.getByTestId('block').children[0].children[0].children[2];
  assert.equal(text.tag, 'text');
  assert.ok(text.children.length > 256 && text.children.length <= 800, `${text.children.length} spans in one text`);
  assert.equal(textOf(text), code);
});

test('plain code past 64 KiB is several texts, each within it', async () => {
  const line = 'x'.repeat(99);
  const code = Array.from({ length: 1000 }, () => line).join('\n');
  const screen = await renderPage(`
    import { CodeBlock } from '@liulidesign/core/Code';
    const code = ${JSON.stringify(code)};
    export function render() { return <page><CodeBlock code={code} label="Big" data-testid="block" /></page>; }
  `);
  const column = screen.getByTestId('block').children[0].children[0];
  assert.equal(column.tag, 'column');
  assert.ok(column.children.length === 2 && column.children.every(text => Buffer.byteLength(textOf(text)) <= 64 * 1024), `${column.children.length} texts`);
  assert.equal(column.children.map(textOf).join('\n'), code);
});

test('every syntax colour reads at 4.5:1 on the block and on each line tint, light and dark', async () => {
  const { roles } = await palette();
  const grounds = [{ pair: ['', 'text/6'], over: 'surface' }, { pair: ['', 'success-soft'] }, { pair: ['', 'danger-soft'] }, { pair: ['', 'accent-soft'] }];
  for (const appearance of ['light', 'dark'])
    for (const role of ['accent', 'chart-5', 'chart-4', 'chart-7', 'text-muted'])
      for (const ground of grounds) {
        const { value } = ratio(roles[appearance], { ...ground, pair: [role, ground.pair[1]] });
        assert.ok(value >= 4.5, `${role} on ${ground.pair[1]}, ${appearance}: ${value.toFixed(2)}`);
      }
});

test('a sideways scroll fades each edge its lines run on under, and none while they fit', async () => {
  const screen = await renderExample('Code/Code', 'Command');
  const scroll = () => screen.getByTestId('install').children[1];
  const masked = () => Object.keys(scroll().props).filter(key => /mask/i.test(key)).map(key => JSON.stringify(scroll().props[key]));
  assert.deepEqual(masked(), [], 'at rest with both edges reached');
  screen.fire(scroll(), 'onScrollEdge', JSON.stringify({ edge: 'end', at: false }));
  const end = masked();
  assert.equal(end.length, 1);
  screen.fire(scroll(), 'onScrollEdge', JSON.stringify({ edge: 'start', at: false }));
  assert.notDeepEqual(masked(), end, 'mid-scroll: both edges');
});
