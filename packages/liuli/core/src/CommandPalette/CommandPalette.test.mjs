import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

const open = async () => {
  const screen = await renderExample('CommandPalette/CommandPalette', 'Commands');
  screen.press(screen.getByTestId('palette-open'));
  return screen;
};
const rows = screen => screen.queryAll(node => node.props.role === 'option');
const highlighted = screen => rows(screen).filter(row => row.props.stateSelected === 1).map(row => row.testId);

test('Commands: a dialog with a search line over groups of results; the first row is highlighted', async () => {
  const screen = await open();
  const palette = screen.getByTestId('palette');
  assert.deepEqual([palette.props.role, palette.props.accessibilityLabel], ['dialog', 'Search commands']);
  assert.deepEqual(screen.queryAll(node => node.props.role === 'group').map(group => group.props.accessibilityLabel), ['Go to', 'Actions']);
  assert.equal(rows(screen).length, 7);
  assert.deepEqual(highlighted(screen), ['palette-all']);
  assert.equal(screen.getByTestId('palette-all').props.accessibilityLabel, 'All notes, G then A');
});

test('Commands: typing narrows the list, best match first; the arrows move the highlight; Return runs and closes', async () => {
  const screen = await open();
  const search = () => screen.getByTestId('palette-search');
  screen.type(search(), 's');
  assert.deepEqual(rows(screen).map(row => row.testId), ['palette-shared', 'palette-settings', 'palette-all', 'palette-share', 'palette-export'], 'from the start of the label first');
  screen.type(search(), 'sh');
  assert.deepEqual(rows(screen).map(row => row.testId), ['palette-shared', 'palette-share']);
  screen.key(search(), 'ArrowDown');
  assert.deepEqual(highlighted(screen), ['palette-share']);
  screen.key(search(), 'Enter');
  assert.equal(screen.queryAll(node => node.testId === 'palette').length, 0);
  screen.getByText('Last run: share');
});

test('Commands: nothing matching is said', async () => {
  const screen = await open();
  screen.type(screen.getByTestId('palette-search'), 'zzz');
  const note = screen.queryAll(node => node.props.role === 'status' && textOf(node) === 'No commands match');
  assert.equal(note.length, 1);
});

test('Commands: End goes to the last row that can be chosen, past the one that cannot', async () => {
  const screen = await open();
  screen.key(screen.getByTestId('palette-search'), 'End');
  assert.deepEqual(highlighted(screen), ['palette-share']);
});

test('At large text: the button that opens the palette stays inside its card and is not cut', async () => {
  const screen = await renderExample('CommandPalette/CommandPalette', 'Commands');
  const open = screen.getByTestId('palette-open');
  assert.deepEqual([open.props.maxWidthPercent, open.props.height, open.props.minHeight > 0], [100, undefined, true]);
  assert.equal(open.children.find(node => node.tag === 'text').props.whiteSpace, undefined, 'its label wraps only when the button alone is wider than the card');
});

test('a disabled first row is not highlighted; with no row that can be chosen nothing is; closed from outside, it opens again empty', async () => {
  const screen = await renderPage(`
    import { useLocal } from '@lingxia/view';
    import { CommandPalette } from '@liulidesign/core/CommandPalette';
    const groups = [{ label: 'Go', items: [{ id: 'locked', label: 'Locked', isDisabled: true }, { id: 'inbox', label: 'Inbox' }, { id: 'trash', label: 'Trash', isDisabled: true }] }];
    export function render() {
      const open = useLocal('open', true);
      return <page>
        <button data-testid="toggle" onClick={() => open.set(!open.value)}><text>Toggle</text></button>
        <CommandPalette isOpen={open.value} onOpenChange={next => open.set(next)} label="Go to" groups={groups} onSelect={() => {}} data-testid="p" />
      </page>;
    }
  `);
  const search = () => screen.getByTestId('p-search');
  assert.deepEqual(highlighted(screen), ['p-inbox'], 'the first row that can be chosen');
  screen.key(search(), 'Home');
  screen.key(search(), 'End');
  assert.deepEqual(highlighted(screen), ['p-inbox']);
  screen.type(search(), 'locked');
  assert.deepEqual(highlighted(screen), [], 'only a disabled row matches');
  assert.equal(search().props.ariaActiveDescendant, undefined);
  for (const name of ['Home', 'End', 'ArrowDown']) screen.key(search(), name);
  assert.equal(search().props.ariaActiveDescendant, undefined);
  screen.press(screen.getByTestId('toggle'));
  screen.press(screen.getByTestId('toggle'));
  assert.equal(search().props.inputValue, '', 'the query does not outlive a close from outside');
  assert.deepEqual(highlighted(screen), ['p-inbox']);
});
