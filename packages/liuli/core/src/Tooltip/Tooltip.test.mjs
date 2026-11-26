import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

test('Toolbar: an IconButton takes the hint itself, shown on interest, never by its own press', async () => {
  const screen = await renderExample('Tooltip/Tooltip', 'Toolbar');
  const undo = screen.getByTestId('undo');
  assert.equal(undo.tag, 'button');
  assert.equal(undo.props.accessibilityLabel, 'Undo');
  assert.ok(undo.props.interestFor, 'shown while a pointer or focus rests on it');
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 3, 'no wrapper around a Liuli button');
  assert.equal(screen.queryAll(node => node.testId === 'undo-content').length, 0);
});

test('OnWords: words are wrapped in one plain button that carries the interest', async () => {
  const screen = await renderExample('Tooltip/Tooltip', 'OnWords');
  const trigger = screen.getByTestId('synced');
  assert.equal(trigger.tag, 'button');
  assert.ok(trigger.props.interestFor);
  assert.equal(trigger.children[0].tag, 'text');
  assert.deepEqual(described(screen, trigger), [], 'not described while the hint is hidden');
});

test('a Link mid-sentence carries the hint and keeps its own press', async () => {
  const screen = await renderPage(`
    import { Tooltip } from '@liulidesign/core/Tooltip';
    import { Link } from '@liulidesign/core/Link';
    export function render() { return <page><text>Due <Tooltip content="Sunday 18:00"><Link label="Sunday" data-testid="when" /></Tooltip>, at six.</text></page>; }
  `);
  const link = screen.getByTestId('when');
  assert.equal(link.tag, 'link');
  assert.ok(link.props.interestFor);
  assert.equal(link.props.stateExpanded, undefined, 'a hint is not a panel the link opens');
});

test('InSentence: the link stays in its sentence and wires the overlay', async () => {
  const screen = await renderExample('Tooltip/Tooltip', 'InSentence');
  const link = screen.queryAll(node => node.tag === 'link')[0];
  assert.ok(link, 'a link');
  assert.equal(screen.queryAll(node => node.tag === 'text' && node.children?.includes(link)).length, 1, 'inside the sentence');
  assert.ok(link.props.interestFor || link.props.stateExpanded === 0, 'wired to its overlay');
});

test('OnWords: words that carry a hint are underlined, dotted; a Button or a Link trigger keeps its own look', async () => {
  const screen = await renderExample('Tooltip/Tooltip', 'OnWords');
  const { textDecorationLine, textDecorationStyle, textDecorationColor } = screen.getByTestId('synced').props;
  assert.deepEqual([textDecorationLine, textDecorationStyle, textDecorationColor], ['underline', 'dotted', 'line-strong']);
  const toolbar = await renderExample('Tooltip/Tooltip', 'Toolbar');
  assert.equal(toolbar.queryAll(node => node.tag === 'button' && node.props.textDecorationStyle === 'dotted').length, 0);
});

test('a Button trigger without a test id of its own takes the tooltip\'s, with its data-*; its own wins', async () => {
  const screen = await renderPage(`
    import { Button } from '@liulidesign/core/Button';
    import { Tooltip } from '@liulidesign/core/Tooltip';
    export function render() {
      return <page>
        <Tooltip content="Saves a copy" data-testid="help" data-kind="hint"><Button label="Save" onClick={() => {}} /></Tooltip>
        <Tooltip content="Prints it" data-testid="print-help"><Button label="Print" onClick={() => {}} data-testid="print" /></Tooltip>
      </page>;
    }
  `);
  const save = screen.getByTestId('help');
  assert.deepEqual([save.tag, save.props.accessibilityLabel, JSON.parse(save.props.dataAttributes).kind], ['button', 'Save', 'hint']);
  assert.equal(screen.getByTestId('print').props.accessibilityLabel, 'Print');
  assert.equal(screen.queryAll(node => node.testId === 'print-help').length, 0);
});

test('a Tooltip as the trigger of a Popover or DropdownMenu: the one button opens the panel and shows the hint', async () => {
  const screen = await renderPage(`
    import { IconButton } from '@liulidesign/core/IconButton';
    import { Popover } from '@liulidesign/core/Popover';
    import { DropdownMenu } from '@liulidesign/core/DropdownMenu';
    import { Tooltip } from '@liulidesign/core/Tooltip';
    export function render() {
      return <page><view className="flex-row">
        <Popover label="Insert" content={<text>Blocks</text>}>
          <Tooltip content="Insert"><IconButton label="Insert" icon="add" data-testid="insert" /></Tooltip>
        </Popover>
        <DropdownMenu label="More" items={[{ id: 'a', label: 'A', onClick: () => {} }]}>
          <Tooltip content="More"><IconButton label="More" icon="more" data-testid="more" /></Tooltip>
        </DropdownMenu>
      </view></page>;
    }
  `);
  for (const [id, shown] of [['insert', 'Blocks'], ['more', 'A']]) {
    assert.ok(screen.getByTestId(id).props.interestFor, `${id} shows its hint`);
    assert.equal(screen.queryAll(node => (node.props.text ?? node.text) === shown).length, 0);
    screen.press(screen.getByTestId(id));
    assert.ok(screen.queryAll(node => (node.props.text ?? node.text) === shown || node.props.accessibilityLabel === shown).length > 0, `${id} opens its panel`);
    screen.escape?.();
  }
  assert.equal(screen.queryAll(node => node.tag === 'button').length, 2, 'no wrapper button around either');
});
