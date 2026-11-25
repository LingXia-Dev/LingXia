import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { render } from '@lingxia/view/testing';

async function withPage(source, run) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-paste-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), source);
    await run(join(root, 'page.tsx'));
  } finally { await rm(root, { recursive: true, force: true }); }
}

const page = `import { useLocal } from '@lingxia/view';
export function render() {
  const doc = useLocal('doc', 'ab'), line = useLocal('line', ''), seen = useLocal('seen', '');
  return <view>
    <textarea data-testid="doc" bind={doc} aria-label="Doc" onPaste={event => {
      seen.set(JSON.stringify({ text: event.text, html: event.html ?? null, files: event.files.length, omitted: event.omitted, selection: event.selection, plain: event.plain, stamped: event.timeStamp > 0 }));
      if (event.html) { event.preventDefault(); doc.replaceRange(event.selection.start, event.selection.end, '<' + event.text + '>'); }
    }} />
    <input data-testid="line" bind={line} aria-label="Line" onPaste={() => {}} />
    <text data-testid="seen">{seen.value}</text>
  </view>;
}
`;

test('onPaste hears the flavours, and preventDefault keeps the default insert out', () => withPage(page, async file => {
  const screen = await render(file, { now: 1_000 });
  const doc = () => screen.getByTestId('doc');
  const seen = () => JSON.parse(screen.getByTestId('seen').children[0]?.text ?? screen.getByTestId('seen').text);
  assert.deepEqual(screen.paste(doc(), { text: '3*4', html: '<b>3*4</b>', selection: { start: 1, end: 1 } }), { prevented: true });
  assert.equal(doc().props.inputValue, 'a<3*4>b', 'the handler inserted its own result, the default did not');
  assert.deepEqual(seen(), { text: '3*4', html: '<b>3*4</b>', files: 0, omitted: [], selection: { start: 1, end: 1 }, plain: false, stamped: true });
  assert.deepEqual(screen.paste(doc(), { text: 'x\r\ny', html: '<i>x</i>', plain: true }), { prevented: false });
  assert.equal(doc().props.inputValue, 'a<3*4>bx\ny', 'a plain paste carries no html, so the default inserts it');
  assert.equal(seen().html, null);
}));

test('a single-line editor takes pasted line breaks as spaces', () => withPage(page, async file => {
  const screen = await render(file);
  const line = () => screen.getByTestId('line');
  assert.deepEqual(screen.paste(line(), { text: 'one\ntwo' }), { prevented: false });
  assert.equal(line().props.inputValue, 'one two');
}));

test('onPaste takes a function', () => withPage(`export function render() { return <input aria-label="x" value="" onPaste={'no' as never} />; }`, async file => {
  await assert.rejects(render(file), /UI_INPUT: onPaste takes a function/);
}));
