import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, zh } from '../testing/harness.mjs';

const labelOf = node => node.props.accessibilityLabel;

test('Lesson: the title is a level-one heading and each block keeps its id', async () => {
  const screen = await renderExample('JinshuView/JinshuView', 'Lesson');
  assert.equal(textOf(screen.getByText('Limits')), 'Limits');
  for (const id of ['h1', 'p1', 'l1', 'c1', 'k1', 'f1']) assert.ok(screen.getByTestId(`lesson-block-${id}`), id);
});

test('Lesson: the file and code chrome speak the page language', async () => {
  const en = await renderExample('JinshuView/JinshuView', 'Lesson');
  assert.ok(en.getByText('12 pages · 470.7 KB'));
  assert.ok(en.getByText('Download'));
  const cn = await renderExample('JinshuView/JinshuView', 'Lesson', zh);
  assert.ok(cn.getByText('12 页 · 470.7 KB'));
  assert.ok(cn.getByText('下载'));
});

test('Resume: the bar names the heading and offers to continue', async () => {
  const en = await renderExample('JinshuView/JinshuView', 'Resume');
  assert.ok(en.getByText('You stopped at “What a limit says”'));
  assert.ok(en.queryAll(node => labelOf(node) === 'Start from the beginning').length > 0);
  const cn = await renderExample('JinshuView/JinshuView', 'Resume', zh);
  assert.ok(cn.getByText('上次读到「What a limit says」'));
  assert.ok(cn.getByText('继续阅读'));
});

test('Unavailable: one line in place of the document', async () => {
  const en = await renderExample('JinshuView/JinshuView', 'Unavailable');
  assert.ok(en.getByText("This document isn't available"));
  const cn = await renderExample('JinshuView/JinshuView', 'Unavailable', zh);
  assert.ok(cn.getByText('文档暂不可用'));
});

test('openAt: the block where a change landed scrolls into view on its first layout', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(
    linked => `import { JinshuView } from ${JSON.stringify(`${linked}/src/JinshuView/JinshuView`)};
import { createDocument, paragraph } from ${JSON.stringify(`${linked}/src/schema/index`)};
const doc = createDocument({ title: 'T', blocks: [paragraph('One.', 'a'), paragraph('Two.', 'b')] });
export function render() {
  return <page><JinshuView doc={doc} openAt="b" /></page>;
}
`,
  );
  const target = screen.queryAll(n => String(n.props.elementId ?? n.props.id ?? '').endsWith('-o-b'));
  assert.equal(target.length, 1);
  assert.ok(target[0].props.onLayout, 'reveals itself once laid out');
  assert.equal(screen.queryAll(n => String(n.props.elementId ?? n.props.id ?? '').endsWith('-o-a')).length, 0);
});

test('One document twice on a page: each instance scopes its element ids', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(
    linked => `import { useLocal } from '@lingxia/view';
import { JinshuView } from ${JSON.stringify(`${linked}/src/JinshuView/JinshuView`)};
import { JinshuEditor } from ${JSON.stringify(`${linked}/src/JinshuEditor/JinshuEditor`)};
import { JinshuDiff } from ${JSON.stringify(`${linked}/src/JinshuDiff/JinshuDiff`)};
import { useJinshuNav } from ${JSON.stringify(`${linked}/src/JinshuOutline/JinshuOutline`)};
import { createDocument, heading, paragraph, table } from ${JSON.stringify(`${linked}/src/schema/index`)};
const a = createDocument({ title: 'T', blocks: [heading(2, 'H', 'h'), paragraph('One.', 'p'), table(['A'], [['1']], { id: 't' })] });
const b = createDocument({ title: 'T', blocks: [heading(2, 'H', 'h'), paragraph('Two.', 'p')] });
export function render() {
  const nav = useJinshuNav('lesson-nav');
  return <page>
    <JinshuView doc={a} nav={nav} />
    <JinshuView doc={a} />
    <JinshuEditor doc={a} onOps={() => {}} hasLeaveGuard={false} />
    <JinshuEditor doc={a} onOps={() => {}} hasLeaveGuard={false} />
    <JinshuDiff a={a} b={b} />
    <JinshuDiff a={a} b={b} />
  </page>;
}
`,
  );
  const ids = screen.queryAll(n => typeof (n.props.elementId ?? n.props.id) === 'string').map(n => n.props.elementId ?? n.props.id);
  assert.deepEqual(ids.filter((id, i) => ids.indexOf(id) !== i), []);
  // The nav's scope names the blocks it scrolls to.
  assert.equal(ids.filter(id => id === 'jinshu-lesson-nav-b-p').length, 1);
});

test('Course: with played-only, a chapter unlocks when the video plays up to it, not when it jumps there', async () => {
  const screen = await renderExample('JinshuView/JinshuView', 'Course');
  const status = { readyState: 'ready', currentTime: 0, duration: 10, paused: false, ended: false, buffering: false, seeking: false,
    buffered: 10, videoWidth: 1280, videoHeight: 720, playbackRate: 1, volume: 1, muted: false, quality: 'auto', qualities: [], error: null };
  const at = s => screen.video(screen.queryAll(node => node.tag === 'video')[0], { ...status, currentTime: s }, [{ type: 'timeupdate', currentTime: s }]);
  const open = title => screen.queryAll(node => labelOf(node)?.endsWith(title))[0].props.enabled !== 0;
  at(9);
  assert.equal(open('Worked examples'), false, 'a jump is not watching');
  for (let s = 0; s <= 3.5; s += 0.5) at(s);
  assert.equal(open('Three conditions'), true);
  assert.equal(open('Worked examples'), false);
});

const pageImports = linked => `import { useLocal } from '@lingxia/view';
import { JinshuView } from ${JSON.stringify(`${linked}/src/JinshuView/JinshuView`)};
import { JinshuOutline, useJinshuNav } from ${JSON.stringify(`${linked}/src/JinshuOutline/JinshuOutline`)};
import { createDocument, details, heading, list, listItem, paragraph, run, video } from ${JSON.stringify(`${linked}/src/schema/index`)};
`;

test('Reading: the scroll reports through the nav; dwell, position and the outline follow it', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(
    linked => `${pageImports(linked)}
const doc = createDocument({ blocks: [heading(2, 'First', 'h1'), paragraph('One.', 'p1'), heading(2, 'Second', 'h2'), paragraph('Two.', 'p2')] });
export function render() {
  const nav = useJinshuNav('r');
  const log = useLocal('log', '');
  return <page>
    <JinshuOutline doc={doc} nav={nav} />
    <scroll data-testid="scroll" onVisibleChange={nav.onVisibleChange}>
      <JinshuView doc={doc} nav={nav} onObserve={events => log.set(log.value + events.map(e => JSON.stringify(e)).join('\\n') + '\\n')} />
    </scroll>
    <text data-testid="log">{log.value}</text>
  </page>;
}
`,
  );
  const scroll = () => screen.getByTestId('scroll');
  const facts = () => textOf(screen.getByTestId('log')).split('\n').filter(Boolean).map(line => JSON.parse(line));
  screen.visible(scroll(), [{ id: 'jinshu-r-b-h1', ratio: 1, top: 0 }, { id: 'jinshu-r-b-p1', ratio: 1, top: 40 }]);
  screen.advance(4000);
  screen.visible(scroll(), [{ id: 'jinshu-r-b-h2', ratio: 1, top: -40 }, { id: 'jinshu-r-b-p2', ratio: 1, top: -5 }]);
  const got = facts();
  const dwell = got.filter(f => f.kind === 'dwell');
  assert.deepEqual(dwell.map(f => [f.blockId, f.dwellMs]), [['h1', 4000], ['p1', 4000]]);
  assert.ok(dwell.every(f => typeof f.contentHash === 'string'), 'dwell names the content it saw');
  assert.deepEqual(got.filter(f => f.kind === 'position').map(f => [f.blockId, f.offset]), [['h1', 0], ['p2', 5]]);
  assert.notEqual(got[0].sessionId, 'view', 'each mount is a session of its own');
  assert.deepEqual(got.map(f => f.seq), got.map((_, i) => i + 1));
  const tinted = screen.queryAll(n => n.tag === 'text' && textOf(n) === 'Second' && n.props.color === 'accent');
  assert.equal(tinted.length, 1, 'the outline tints the heading being read');
});

test('In-page links scroll without a nav, opening the fold that holds a list item', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(
    linked => `${pageImports(linked)}
const doc = createDocument({ blocks: [
  paragraph([run('See '), run('the item', { link: '#i1' })], 'p1'),
  details('More', [list('bullet', [listItem('Hidden item', { id: 'i1' })], { id: 'l1' })], { id: 'd1' }),
] });
export function render() {
  return <page><JinshuView doc={doc} /></page>;
}
`,
  );
  const fold = () => screen.queryAll(n => n.tag === 'button' && textOf(n) === 'More')[0];
  assert.equal(fold().props.stateExpanded, 0, 'folded at first');
  screen.press(screen.queryAll(n => n.tag === 'link')[0]);
  assert.equal(fold().props.stateExpanded, 1);
  assert.equal(screen.queryAll(n => String(n.props.elementId ?? n.props.id ?? '').endsWith('-b-i1')).length, 1, 'the item carries the id the link reveals');
});

test('Played-only resume: what was watched before stays open, for the same source only', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const page = src => linked => `${pageImports(linked)}
const doc = createDocument({ blocks: [video('https://example.com/a.mp4', { durationMs: 10_000, chapters: [{ title: 'Start', atMs: 0 }, { title: 'Later', atMs: 6_000 }], id: 'v1' })] });
const resume = { blocks: {}, media: { v1: { src: ${JSON.stringify(src)}, played: [[0, 6500]], positionMs: 6500, durationMs: 10_000 } } };
export function render() {
  return <page><JinshuView doc={doc} resume={resume} mediaPolicy={{ seek: 'played-only' }} /></page>;
}
`;
  const open = (screen, title) => screen.queryAll(node => labelOf(node)?.endsWith(title))[0].props.enabled !== 0;
  assert.equal(open(await renderPage(page('https://example.com/a.mp4')), 'Later'), true);
  assert.equal(open(await renderPage(page('https://example.com/b.mp4')), 'Later'), false, 'a replaced video starts over');
});
