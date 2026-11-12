import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { render, textOf } from '@lingxia/view/testing';

const counter = `import { useLocal } from '@lingxia/view';
export function Counter({ label }: { label: string }) {
  const count = useLocal('count', 0);
  return <view className="flex-row gap-2">
    <text data-testid="value">{label}: {count.value}</text>
    <button data-testid="inc" onClick={() => count.set(count.value + 1)}>Add</button>
    <view className="gap-1"><text data-testid="note">Twice</text><text data-testid="note">Twice</text></view>
  </view>;
}
export function render(view: { data: { label?: string } }) { return <Counter label={view.data.label ?? 'Count'} />; }
`;

async function withPage(run) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-testing-entry-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), counter);
    await run(join(root, 'page.tsx'));
  } finally { await rm(root, { recursive: true, force: true }); }
}

test('a compiled page renders, takes presses and re-renders with its local state', () => withPage(async page => {
  const screen = await render(page, { data: { label: 'Taps' } });
  assert.equal(textOf(screen.getByTestId('value')), 'Taps: 0');
  assert.equal(screen.root.props.flexDirection, 'row');
  screen.press(screen.getByTestId('inc'));
  screen.press(screen.getByText('Add'));
  assert.equal(textOf(screen.getByTestId('value')), 'Taps: 2');
  screen.rerender({ data: { label: 'Hits' } });
  assert.equal(textOf(screen.getByTestId('value')), 'Hits: 2');
  assert.ok(Array.isArray(screen.wire()));
}));

test('each render has its own runtime', () => withPage(async page => {
  const first = await render(page), second = await render(page);
  first.press(first.getByTestId('inc'));
  assert.equal(textOf(first.getByTestId('value')), 'Count: 1');
  assert.equal(textOf(second.getByTestId('value')), 'Count: 0');
}));

test('a query that misses lists what is on screen; one that matches twice names both', () => withPage(async page => {
  const screen = await render(page);
  assert.throws(() => screen.getByTestId('save'), { message: 'no node has data-testid="save"; on screen: value, inc, note' });
  assert.throws(() => screen.getByTestId('note'), {
    message: '2 nodes have data-testid="note":\n  <text data-testid="note"> "Twice" at view > view[2] > text[0]\n  <text data-testid="note"> "Twice" at view > view[2] > text[1]',
  });
  assert.throws(() => screen.getByText('Remove'), /^Error: no node has text "Remove"; on screen: .*"Add"/);
  assert.throws(() => screen.press(screen.getByTestId('value')), { message: '<text data-testid="value"> "Count: 0" has no onClick' });
}));
