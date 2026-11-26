import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('ViewOptions: a named trigger opens a panel named by its title, with live controls', async () => {
  const screen = await renderExample('Popover/Popover', 'ViewOptions');
  const trigger = screen.getByTestId('options');
  assert.equal(trigger.tag, 'button');
  assert.equal(trigger.props.accessibilityLabel, 'View options');
  assert.equal(trigger.props.stateExpanded, 0);
  assert.equal(screen.queryAll(node => node.testId === 'options-panel').length, 0);
  screen.press(trigger);
  assert.equal(screen.getByTestId('options').props.stateExpanded, 1);
  const panel = screen.getByTestId('options-panel');
  assert.equal(panel.props.accessibilityLabel, 'Show in the list');
  assert.equal(panel.props.role, 'dialog');
  assert.equal(panel.props.modal, undefined, 'a non-modal dialog: Tab can leave it');
  assert.equal(screen.queryAll(node => node.props.role === 'heading').map(textOf).includes('Show in the list'), true);
  screen.toggle(screen.getByTestId('dates'));
  assert.equal(textOf(screen.getByTestId('summary')), 'Previews and Dates');
  assert.equal(screen.queryAll(node => node.testId === 'options-panel').length, 1, 'a control inside leaves it open');
  screen.escape();
  assert.equal(screen.queryAll(node => node.testId === 'options-panel').length, 0);
});

test('Share: opens above the trigger, with the gap on its lower side', async () => {
  const screen = await renderExample('Popover/Popover', 'Share');
  screen.press(screen.getByTestId('share'));
  const panel = screen.getByTestId('share-panel');
  assert.equal(panel.props.marginBottom, 6);
  assert.equal(panel.props.marginTop, undefined);
  screen.press(screen.getByTestId('copy'));
  screen.getByText('Copied');
});

test('Beside: the panel is anchored at the end side of the trigger, their tops lined up', async () => {
  const screen = await renderExample('Popover/Popover', 'Beside');
  screen.press(screen.getByTestId('beside'));
  assert.ok(JSON.stringify(screen.getByTestId('beside-panel').props).includes('inline-end span-block-end'));
});

test('a Link mid-sentence opens the panel, anchored to the link', async () => {
  const screen = await renderPage(`
    import { Popover } from '@liulidesign/core/Popover';
    import { Link } from '@liulidesign/core/Link';
    export function render() { return <page><text>Due <Popover label="When" data-testid="due" content={<text>Sunday 18:00</text>}><Link label="Sunday" data-testid="when" /></Popover>, at six.</text></page>; }
  `);
  const link = screen.getByTestId('when');
  assert.equal(link.tag, 'link');
  assert.equal(link.props.stateExpanded, 0);
  screen.press(link);
  assert.equal(screen.getByTestId('when').props.stateExpanded, 1);
  assert.equal(screen.getByTestId('due-panel').props.anchorTo, screen.getByTestId('when').props.anchorName);
});

test('InSentence: the link stays in its sentence and wires the overlay', async () => {
  const screen = await renderExample('Popover/Popover', 'InSentence');
  const link = screen.queryAll(node => node.tag === 'link')[0];
  assert.ok(link, 'a link');
  assert.equal(screen.queryAll(node => node.tag === 'text' && node.children?.includes(link)).length, 1, 'inside the sentence');
  assert.ok(link.props.interestFor || link.props.stateExpanded === 0, 'wired to its overlay');
});

test('Anchored: with `anchor` there is no trigger; the caller opens the panel against that element, and its own closing reaches the caller', async () => {
  const screen = await renderExample('Popover/Popover', 'Anchored');
  assert.throws(() => screen.getByTestId('conflicts'), /no node/, 'no trigger of its own');
  assert.equal(screen.queryAll(node => node.props.role === 'dialog').length, 0);
  screen.press(screen.getByTestId('explain'));
  const panel = screen.getByTestId('conflicts-panel');
  assert.deepEqual([typeof panel.props.anchorTo, panel.props.role, panel.props.accessibilityLabel], ['string', 'dialog', '3 photos changed in two places'], 'placed against the badge, named by its title');
  screen.escape();
  assert.equal(screen.queryAll(node => node.props.role === 'dialog').length, 0, 'Escape closes it and the caller hears');
  screen.press(screen.getByTestId('explain'));
  assert.equal(screen.queryAll(node => node.props.role === 'dialog').length, 1, 'and it opens again');
});

test('anchored, with no trigger, the panel carries the popover\'s data-* props', async () => {
  const screen = await renderPage(`
    import { Popover } from '@liulidesign/core/Popover';
    export function render() { return <page><view id="spot" /><Popover label="Tip" anchor="spot" isOpen onOpenChange={() => {}} data-testid="tip" data-kind="hint" content={<text>Hi</text>} /></page>; }`);
  assert.deepEqual(JSON.parse(screen.getByTestId('tip-panel').props.dataAttributes), { kind: 'hint' });
});
