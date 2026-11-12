import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Variants: each button is named by its label and is not busy at rest', async () => {
  const screen = await renderExample('Button/Button', 'Variants');
  const buttons = screen.queryAll(node => node.tag === 'button');
  assert.deepEqual(buttons.map(node => node.props.accessibilityLabel), ['Save changes', 'Preview', 'Skip', 'Delete draft']);
  assert.ok(buttons.every(node => !node.props.stateBusy));
  const primary = screen.getByTestId('primary');
  assert.equal(primary.props.background, 'accent');
  assert.equal(primary.props.color, 'on-accent');
});

test('Sizes: heights come from the control tokens', async () => {
  const screen = await renderExample('Button/Button', 'Sizes');
  const heights = screen.queryAll(node => node.tag === 'button').map(node => node.props.minHeight);
  assert.deepEqual(heights, [32, 40, 48]);
});

test('DisabledWithReason: the reason joins the name only while disabled', async () => {
  const screen = await renderExample('Button/Button', 'DisabledWithReason');
  assert.equal(screen.getByTestId('continue').props.accessibilityLabel, 'Continue, Accept the terms first');
  assert.equal(screen.getByTestId('continue').props.onClick, undefined, 'a disabled button takes no press');
  screen.press(screen.getByTestId('toggle-terms'));
  assert.equal(screen.getByTestId('continue').props.accessibilityLabel, 'Continue');
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
