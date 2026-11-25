import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile, rm } from 'node:fs/promises';
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

test('a page using t renders the catalogue text when given the compiled messages', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-testing-messages-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), `import { t } from '@lingxia/view';
export function render() { return <text data-testid="title">{t('cart.title')}</text>; }
`);
    // As `lingxia build` leaves it: a content-named copy and the `current` file naming it.
    const build = join(root, '.lingxia/i18n/build');
    await mkdir(join(build, 'c0ffee'), { recursive: true });
    await writeFile(join(build, 'current'), 'c0ffee');
    await writeFile(join(build, 'c0ffee/index.js'), `export const source = 'en-US';
export const languages = { 'en-US': { 'cart.title': () => 'Your cart' } };
export const formatter = () => ({ j: parts => parts.join('') });
`);
    for (const messages of ['.lingxia/i18n/build', '.lingxia/i18n/build/c0ffee/index.js']) {
      const screen = await render(join(root, 'page.tsx'), {}, { messages });
      assert.equal(textOf(screen.getByTestId('title')), 'Your cart', messages);
    }
    await assert.rejects(render(join(root, 'page.tsx')), /UI_I18N/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('a channels double serves the page\'s useChannel through its own bridge, both ways', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-testing-channels-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), `import { useChannel, useLocal, useView } from '@lingxia/view';
export function render() {
  const { actions } = useView();
  const lines = useLocal<string[]>('lines', []);
  const feed = useChannel('feed', actions.feed, { params: { room: 'a' }, onData: line => lines.set(latest => [...latest, String(line)]) });
  return <view>
    <text data-testid="state">{feed.connected ? 'open' : 'closed'}: {lines.value.join(',')}</text>
    <button data-testid="say" onClick={() => feed.send('hi')}>Say</button>
    <link data-testid="away" to={{ page: 'other' }}><text>Away</text></link>
  </view>;
}
`);
    const heard = [], opened = [];
    let logic;
    const screen = await render(join(root, 'page.tsx'), { channels: { feed(channel, params) {
      logic = channel; opened.push(params);
      channel.on('data', payload => { heard.push(payload); channel.send(`echo ${payload}`); });
      channel.send('welcome');
    } } });
    await screen.settle();
    assert.deepEqual(opened, [{ room: 'a' }]);
    assert.equal(textOf(screen.getByTestId('state')), 'open: welcome');
    screen.press(screen.getByTestId('say'));
    await screen.settle();
    assert.deepEqual(heard, ['hi']);
    assert.equal(textOf(screen.getByTestId('state')), 'open: welcome,echo hi');
    logic.close();
    await screen.settle();
    assert.equal(textOf(screen.getByTestId('state')), 'closed: welcome,echo hi');
    screen.press(screen.getByTestId('away'));
    assert.deepEqual(screen.takeRequests().map(message => message.method), ['nav.go']);
    assert.throws(() => screen.rerender({ channels: {} }), /served from the first render/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

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

test('screen.visible reports a List row by its key and a scroll child by its id, with the current one', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-testing-visible-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), `import { List, useLocal } from '@lingxia/view';
const rows = Object.freeze(['a', 'b', 'c']);
export function render() {
  const seen = useLocal('seen', '');
  const say = (event: { items: readonly { id: string }[]; current?: string; timeStamp: number }) =>
    seen.set(event.items.map(item => item.id).join(',') + ' @' + (event.current ?? '-') + (event.timeStamp > 0 ? '' : ' no time'));
  return <view>
    <List data-testid="list" items={rows} getKey={row => 'row-' + row} renderItem={row => <text>{row}</text>} onVisibleChange={say} />
    <scroll data-testid="scroll" onVisibleChange={say}><view id="intro" /></scroll>
    <text data-testid="seen">{seen.value}</text>
  </view>;
}
`);
    const screen = await render(join(root, 'page.tsx'));
    screen.visible(screen.getByTestId('list'), [{ index: 1, ratio: 0.5, top: -10 }, { index: 2, ratio: 1, top: 40 }]);
    assert.equal(textOf(screen.getByTestId('seen')), 'row-b,row-c @row-b');
    screen.visible(screen.getByTestId('scroll'), [{ id: 'intro', ratio: 1, top: 8 }]);
    assert.equal(textOf(screen.getByTestId('seen')), 'intro @intro');
    screen.visible(screen.getByTestId('scroll'), []);
    assert.equal(textOf(screen.getByTestId('seen')), ' @-');
  } finally { await rm(root, { recursive: true, force: true }); }
});
