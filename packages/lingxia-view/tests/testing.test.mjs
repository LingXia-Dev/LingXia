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

const controls = `import { useLocal } from '@lingxia/view';
export function render() {
  const log = useLocal('log', '');
  const on = useLocal('on', false), stuck = useLocal('stuck', false), size = useLocal('size', 's'), seats = useLocal('seats', 1);
  const page = useLocal('page', 0), at = useLocal('at', 0), open = useLocal('open', true);
  const note = (entry: string) => log.set(log.value + entry + ';');
  return <page className="h-[50vh]">
    <text data-testid="log">{log.value}</text>
    <checkbox data-testid="on" value={on.value} onChange={(next: boolean) => on.set(next)} aria-label="On" />
    <checkbox data-testid="stuck" value={stuck.value} onChange={() => {}} aria-label="Stuck" />
    <select data-testid="size" value={size.value} onChange={(next: string) => size.set(next)} aria-label="Size"
      options={[{ value: 's', label: 'Small' }, { value: 'l', label: 'Large' }]} />
    <slider data-testid="seats" bind={seats} min={1} max={10} aria-label="Seats" />
    <input data-testid="name" aria-label="Name" onInput={(text: string) => note('input ' + text)} onSubmit={(text: string) => note('submit ' + text)} />
    <scroll data-testid="feed" className="snap-x" bind={page} onReachEnd={() => note('end')}>
      <view className="snap-start w-10 h-10" /><view className="snap-start w-10 h-10" />
    </scroll>
    <image data-testid="photo" src="images/a.png" alt="A" onLoad={() => note('loaded')} />
    <plot data-testid="chart" kind="bar" values={[1, 2]} aria-label="Sales" onSelect={(index: number) => note('point ' + index)} />
    <view data-testid="card" drag="x" dragSnapPoints={[0, 120]} bind={at} aria-label="Card" />
    <video data-testid="clip" src="https://example.com/a.mp4" onPlay={() => note('play')} />
    <text data-testid="state">{[on.value, size.value, seats.value, page.value, at.value].join(' ')}</text>
    <dialog open={open.value} onClose={() => open.set(false)} closedby="any" aria-label="Hello"><text>Hi</text></dialog>
  </page>;
}
`;

test('every event the elements define is reachable from the screen, with a phone environment by default', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-testing-entry-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), controls);
    const screen = await render(join(root, 'page.tsx'));
    const get = id => screen.getByTestId(id);
    screen.toggle(get('on'), true);
    assert.throws(() => screen.toggle(get('on'), true), { message: '<checkbox data-testid="on"> is already checked' });
    assert.throws(() => screen.toggle(get('stuck'), true), { message: '<checkbox data-testid="stuck"> stayed unchecked: its onChange did not update value' });
    screen.select(get('size'), 'l');
    screen.slide(get('seats'), 4);
    screen.type(get('name'), 'Ada');
    screen.submit(get('name'), 'Ada');
    screen.snap(get('feed'), 1);
    screen.reach(get('feed'), 'end');
    screen.image(get('photo'), 'load');
    screen.selectPoint(get('chart'), 1);
    screen.drag(get('card'), { phase: 'settle', index: 1 });
    screen.video(get('clip'), {}, [{ type: 'play' }]);
    assert.equal(textOf(get('state')), 'true l 4 1 1');
    assert.equal(textOf(get('log')), 'input Ada;submit Ada;end;loaded;point 1;play;');
    assert.equal(screen.escape(), true);
    assert.equal(screen.escape(), false);
    assert.equal(typeof screen.host.__lingxiaDispatch, 'function');
    screen.rerender({ env: { viewport: { width: 800, height: 600 }, theme: 'dark' } });
  } finally { await rm(root, { recursive: true, force: true }); }
});
