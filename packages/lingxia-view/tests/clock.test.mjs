import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { render, textOf } from '@lingxia/view/testing';

async function withPage(source, run) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-clock-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), source);
    await run(join(root, 'page.tsx'));
  } finally { await rm(root, { recursive: true, force: true }); }
}

const at = Date.UTC(2026, 0, 1, 10, 0, 30);

test('useNow holds the pinned minute and re-renders only when advance crosses a step', () => withPage(`import { useNow, useLocal } from '@lingxia/view';
let renders = 0;
function Clock() { renders++; return <text data-testid="clock">{new Date(useNow('minute')).toISOString().slice(11, 19)}</text>; }
export function render() {
  const stamp = useLocal('stamp', 0);
  return <view><Clock /><button data-testid="tap" onClick={event => stamp.set(event.timeStamp)}>Tap</button><text data-testid="stamp">{String(stamp.value)}</text><text data-testid="renders">{String(renders)}</text></view>;
}
`, async page => {
  const screen = await render(page, { now: at });
  assert.equal(textOf(screen.getByTestId('clock')), '10:00:00');
  const renders = () => textOf(screen.getByTestId('renders'));
  const before = renders();
  screen.advance(20_000);
  assert.equal(textOf(screen.getByTestId('clock')), '10:00:00');
  assert.equal(renders(), before, 'a move inside the minute renders nothing');
  screen.advance(10_000);
  assert.equal(textOf(screen.getByTestId('clock')), '10:01:00');
  screen.press(screen.getByTestId('tap'));
  assert.equal(textOf(screen.getByTestId('stamp')), String(at + 30_000), 'a press is stamped with the test clock');
  // The press re-rendered the page around a recalled Clock, which still reads the minute.
  screen.advance(60_000);
  assert.equal(textOf(screen.getByTestId('clock')), '10:02:00');
}));

test('useNow by the second steps each second, and a page that never reads it ignores the clock', async () => {
  await withPage(`import { useNow } from '@lingxia/view';
export function render() { return <text data-testid="t">{String((useNow('second') / 1000) % 60)}</text>; }
`, async page => {
    const screen = await render(page, { now: at + 400 });
    assert.equal(textOf(screen.getByTestId('t')), '30');
    screen.advance(600);
    assert.equal(textOf(screen.getByTestId('t')), '31');
  });
  await withPage(`import { useNow } from '@lingxia/view';
export function render() { return <text data-testid="t">{String(typeof useNow)}</text>; }
`, async page => {
    const screen = await render(page, { now: at });
    screen.advance(120_000);
    assert.equal(textOf(screen.getByTestId('t')), 'function');
  });
});

test('useNow refuses another granularity', () => withPage(`import { useNow } from '@lingxia/view';
export function render() { return <text>{String(useNow('hour' as 'minute'))}</text>; }
`, async page => {
  await assert.rejects(render(page), /UI_PROP: useNow takes "second" or "minute"/);
}));
