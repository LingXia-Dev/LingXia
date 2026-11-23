import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Variants: each button is named by its label and is not busy at rest', async () => {
  const screen = await renderExample('Button/Button', 'Variants');
  const buttons = screen.queryAll(node => node.tag === 'button');
  assert.deepEqual(buttons.map(node => node.props.accessibilityLabel), ['Save changes', 'Preview', 'Turn off internet', 'Skip', 'Delete draft']);
  assert.ok(buttons.every(node => !node.props.stateBusy));
  const primary = screen.getByTestId('primary');
  assert.equal(primary.props.background, 'accent-solid');
  assert.equal(primary.props.color, 'on-accent-solid');
});

test('Sizes: heights come from the control tokens', async () => {
  const screen = await renderExample('Button/Button', 'Sizes');
  const heights = screen.queryAll(node => node.tag === 'button').map(node => node.props.minHeight);
  assert.deepEqual(heights, [32, 40, 48]);
});

test('DisabledWithReason: the name stays the label; the reason is its description only while disabled', async () => {
  const screen = await renderExample('Button/Button', 'DisabledWithReason');
  const button = () => screen.getByTestId('continue');
  assert.equal(button().props.accessibilityLabel, 'Continue');
  assert.equal(button().props.onClick, undefined, 'a disabled button takes no press');
  const reason = screen.queryAll(node => node.props.elementId === button().props.ariaDescribedBy);
  assert.equal(reason.length, 1);
  assert.equal(textOf(reason[0]), 'Accept the terms first');
  assert.equal(reason[0].props.accessibilityHidden, 1, 'read once, as the description');
  screen.press(screen.getByTestId('toggle-terms'));
  assert.equal(button().props.accessibilityLabel, 'Continue');
  assert.equal(button().props.ariaDescribedBy, undefined);
  assert.equal(screen.queryAll(node => textOf(node) === 'Accept the terms first').length, 0);
});

test('clickAction: busy with a spinner until the action settles, and presses meanwhile are ignored', async () => {
  const screen = await renderPage(`
    import { Button } from '@liulidesign/core/Button';
    let settle: () => void = () => {};
    let calls = 0;
    const action = () => { calls++; return new Promise<void>(resolve => { settle = resolve; }); };
    export function render() {
      return <page>
        <Button label="Publish" clickAction={action} data-testid="publish" />
        <button data-testid="settle" onClick={() => settle()}>Settle</button>
        <text data-testid="calls">{String(calls)}</text>
      </page>;
    }
  `);
  screen.press(screen.getByTestId('publish'));
  assert.equal(screen.getByTestId('publish').props.stateBusy, 1);
  assert.ok(screen.getByTestId('publish').children.some(node => node.tag === 'vector'), 'spinner');
  assert.equal(screen.getByTestId('publish').props.accessibilityLabel, 'Publish', 'the name holds while busy');
  screen.press(screen.getByTestId('publish'));
  screen.press(screen.getByTestId('settle'));
  await new Promise(resolve => setImmediate(resolve));
  screen.rerender({});
  assert.equal(screen.getByTestId('publish').props.stateBusy, undefined);
  assert.equal(screen.getByText('1').tag, 'text', 'the action ran once');
});

test('disabled: a neutral fill with muted text for every variant, never a faded accent', async () => {
  const screen = await renderPage(`
    import { Button } from '@liulidesign/core/Button';
    export function render() {
      return <page>
        <Button label="A" variant="primary" isDisabled data-testid="primary" />
        <Button label="B" variant="destructive" isDisabled data-testid="destructive" />
        <Button label="C" isDisabled data-testid="secondary" />
        <Button label="D" variant="primary" data-testid="enabled" />
      </page>;
    }
  `);
  for (const id of ['primary', 'destructive', 'secondary']) {
    const button = screen.getByTestId(id);
    assert.deepEqual([button.props.background, button.props.color, button.props.opacity], ['text/6', 'text-muted', undefined], id);
  }
  const enabled = screen.getByTestId('enabled');
  assert.equal(enabled.props.borderTopWidth, screen.getByTestId('primary').props.borderTopWidth, 'same box size either way');
});

test('labels start-align when they wrap, and a content-width button in a column sits at the start', async () => {
  const screen = await renderExample('Button/Button', 'DisabledWithReason');
  assert.equal(screen.getByTestId('continue').children[0].props.textAlign, 'start');
  assert.equal(screen.getByTestId('toggle-terms').props.alignSelf, 'start');
});

test('link: accent text at regular weight, no fill at rest', async () => {
  const screen = await renderExample('Button/Button', 'VariantLink');
  const link = screen.getByTestId('forgot');
  assert.equal(link.props.color, 'accent');
  assert.equal(screen.getByText('Forgot password?').props.fontWeight, 400);
});

test('WithIcons: a start icon comes before the label and an end icon after it, both by name and unread; the label stays the name', async () => {
  const screen = await renderExample('Button/Button', 'WithIcons');
  const parts = id => screen.getByTestId(id).children.map(child => child.tag);
  assert.deepEqual(parts('new-note'), ['vector', 'text']);
  assert.deepEqual(parts('continue-on'), ['text', 'vector']);
  const icon = screen.getByTestId('new-note').children[0];
  assert.deepEqual([icon.props.accessibilityHidden, icon.props.width], [1, 18]);
  assert.equal(screen.getByTestId('new-note').props.accessibilityLabel, 'New note');
  assert.match(screen.getByTestId('continue-on').children[1].props.vectorSource, /M5 12h14/, 'the set\'s arrow');
});

test('Floating: the raised shadow in place of the variant\'s hairline one', async () => {
  const button = (await renderExample('Button/Button', 'Floating')).getByTestId('floating');
  assert.deepEqual(JSON.parse(button.props.boxShadow).map(layer => layer.blur), [2, 24]);
});

test('At large text: rows of buttons wrap; a button grows in height and its neighbours never squeeze it', async () => {
  for (const example of ['Variants', 'Hero', 'VariantsEveryday', 'Specimen']) {
    const screen = await renderExample('Button/Button', example);
    const button = screen.queryAll(node => node.tag === 'button')[0];
    const row = screen.queryAll(node => node.children.includes(button))[0];
    assert.equal(row.props.flexWrap, 'wrap', example);
    assert.deepEqual([button.props.shrink, button.props.maxWidthPercent, button.props.height, button.props.minHeight > 0], [0, 100, undefined, true], example);
    assert.equal(button.children.find(node => node.tag === 'text').props.numberOfLines, undefined, example);
  }
});

test('edge: a ghost button at the end of a row hangs its padding past the edge, and an end icon its drawing\'s inset too', async () => {
  const screen = await renderPage(`
    import { Button } from '@liulidesign/core/Button';
    export function render() {
      return <page>
        <Button label="System" variant="ghost" size="sm" endIcon="chevron-down" edge="end" data-testid="menu" />
        <Button label="Edit" variant="ghost" size="md" edge="end" data-testid="edit" />
        <Button label="Back" variant="ghost" size="sm" startIcon="chevron-start" edge="start" data-testid="back" />
        <Button label="Plain" variant="ghost" size="sm" data-testid="plain" />
        <Button label="More" variant="ghost" size="sm" startIcon="more" isIconOnly edge="end" data-testid="square" />
      </page>;
    }
  `);
  const margins = id => { const { marginInlineStart, marginInlineEnd } = screen.getByTestId(id).props; return [marginInlineStart, marginInlineEnd]; };
  assert.deepEqual(margins('menu'), [undefined, -16.33]);
  assert.deepEqual(margins('edit'), [undefined, -17]);
  assert.deepEqual(margins('back'), [-16.33, undefined]);
  assert.deepEqual(margins('plain'), [undefined, undefined]);
  assert.deepEqual(margins('square'), [undefined, -11.33], 'a square hangs its inset around the glyph, plus the drawing\'s');
});

test('secondary: the page\'s surface on a page, the overlay\'s in a dialog\'s footer, where the page\'s would read as a hole', async () => {
  const screen = await renderPage(`
    import { Button } from '@liulidesign/core/Button';
    import { Dialog } from '@liulidesign/core/Dialog';
    export function render() { return <page>
      <Button label="Share" data-testid="page" />
      <Dialog isOpen onOpenChange={() => {}} label="Share" actions={<><Button label="Cancel" data-testid="footer" /><Button label="Send" variant="primary" data-testid="primary" /></>} />
    </page>; }`);
  assert.equal(screen.getByTestId('page').props.background, 'surface');
  assert.equal(screen.getByTestId('footer').props.background, 'surface-overlay');
  assert.equal(screen.getByTestId('primary').props.background, 'accent-solid', 'a solid keeps its own fill');
});

test('tonal: a soft ink fill with no edge or shadow, firmer pressed; disabled keeps the fill and mutes the label', async () => {
  const screen = await renderExample('Button/Button', 'Variants');
  const tonal = screen.getByTestId('tonal');
  assert.equal(tonal.props.background, 'text/6');
  assert.equal(tonal.props.borderColor, '#00000000');
  assert.equal(tonal.props.boxShadow, undefined);
  const disabled = await renderPage(`
    import { Button } from '@liulidesign/core/Button';
    export function render() { return <page><Button label="Turn off" variant="tonal" isDisabled data-testid="off" /></page>; }
  `);
  assert.equal(disabled.getByTestId('off').props.background, 'text/6');
});

test('ToPage: with to the button is one block link, named by its label, that a press opens', async () => {
  const link = (await renderExample('Button/Button', 'ToPage')).getByTestId('to-settings');
  assert.equal(link.tag, 'link-box');
  assert.equal(link.props.accessibilityLabel, 'Open settings');
  assert.equal(typeof link.props.onClick, 'number');
});
