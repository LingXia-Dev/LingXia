import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createDocument, heading, paragraph, plainText } from './schema/index.ts';
import { createJinshuSource } from './source/index.ts';
import { toolsOver } from './testing/agent.mjs';

const lesson = () =>
  createDocument({
    title: 'Week 3',
    blocks: [heading(1, 'Week 3', 'h'), paragraph('First paragraph.', 'p1'), paragraph('Second paragraph.', 'p2')],
  });

const HUMAN = { kind: 'human', id: 'editor' };
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));

function fakeChannel() {
  const sent = [];
  const handlers = { data: new Set(), close: new Set() };
  return {
    sent,
    send: msg => sent.push(msg),
    on(event, fn) {
      handlers[event].add(fn);
      return () => handlers[event].delete(fn);
    },
    emit: msg => [...handlers.data].forEach(fn => fn(msg)),
    close: () => [...handlers.close].forEach(fn => fn({ code: 'closed', reason: '' })),
  };
}

const agentEdit = (source, id, find, replace) =>
  toolsOver({ w3: source }).call('jinshu_edit', { doc: 'w3', base: source.getState().revision, ops: [{ op: 'edit', id, find, replace }] });
const batchesIn = ch => ch.sent.filter(m => m.t === 'agentBatches').flatMap(m => m.batches);

describe('changed while away: only an editor takes the notice', () => {
  it('a reader opening first leaves the away batches for the editor', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    await agentEdit(source, 'p1', 'First', 'Away');
    const reader = fakeChannel();
    source.serve(reader, { doc: 'w3', role: 'reader' });
    assert.equal(batchesIn(reader).length, 0);
    const editor = fakeChannel();
    source.serve(editor, { doc: 'w3', role: 'editor' });
    const [away] = batchesIn(editor);
    assert.equal(away.away, true);
    assert.deepEqual(away.blockIds, ['p1']);
    // Delivered once: a second editor does not see it again.
    const second = fakeChannel();
    source.serve(second, { doc: 'w3', role: 'editor' });
    assert.equal(batchesIn(second).length, 0);
  });

  it('a change while only a reader shows the document waits for an editor, not marked away', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const reader = fakeChannel();
    source.serve(reader, { role: 'reader' });
    await agentEdit(source, 'p2', 'Second', 'Later');
    assert.equal(batchesIn(reader).length, 0);
    const editor = fakeChannel();
    source.serve(editor);
    editor.emit({ t: 'open', doc: 'w3', role: 'editor' });
    const [held] = batchesIn(editor);
    assert.deepEqual(held.blockIds, ['p2']);
    assert.equal(held.away, undefined);
  });

  it('with an editor open, a change goes out live', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const editor = fakeChannel();
    source.serve(editor, { role: 'editor' });
    await agentEdit(source, 'p1', 'First', 'Live');
    assert.equal(batchesIn(editor).length, 1);
    editor.close();
    await agentEdit(source, 'p2', 'Second', 'Then');
    const next = fakeChannel();
    source.serve(next, { role: 'editor' });
    assert.deepEqual(batchesIn(next).map(b => b.blockIds[0]), ['p2']);
  });
});

describe("saveMode: 'auto'", () => {
  it('saves once the person stops editing, not after an agent edit alone', async () => {
    const saves = [];
    const source = createJinshuSource({
      key: 'w3',
      initialDoc: lesson(),
      saveMode: 'auto',
      save: async doc => {
        saves.push(plainText(doc.blocks[1].runs));
        return { ok: true, version: `v${saves.length + 1}` };
      },
    });
    await agentEdit(source, 'p2', 'Second', 'Agent');
    await wait(1700);
    assert.equal(saves.length, 0, 'an agent batch alone does not save');
    source.apply({ batchId: 'a', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'One' }] });
    await wait(700);
    source.apply({ batchId: 'b', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: 'p1', find: 'One', replace: 'Two' }] });
    await wait(1000);
    assert.equal(saves.length, 0, 'still typing');
    await wait(700);
    assert.deepEqual(saves, ['Two paragraph.']);
    assert.equal(source.getState().saveStatus, 'saved');
  });

  it('waits out an open conflict, and retries a failed save on the next edit', async () => {
    let attempt = 0;
    const source = createJinshuSource({
      key: 'w3',
      initialDoc: lesson(),
      saveMode: 'auto',
      save: async () => {
        attempt += 1;
        if (attempt === 1) return { ok: false, error: 'offline' };
        if (attempt === 2) return { ok: false, conflict: true, remoteDoc: lesson(), remoteVersion: 'v9' };
        return { ok: true, version: `v${attempt}` };
      },
    });
    const edit = (find, replace) =>
      source.apply({ batchId: `e${replace}`, origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: 'p1', find, replace }] });
    edit('First', 'One');
    await wait(1700);
    assert.equal(source.getState().saveStatus, 'failed');
    assert.equal(attempt, 1);
    edit('One', 'Two');
    await wait(1700);
    assert.equal(attempt, 2);
    assert.equal(source.getState().conflicts.length, 1);
    edit('Two', 'Three');
    await wait(1700);
    assert.equal(attempt, 2, 'no save while the conflict is open');
    await source.resolveConflict('mine');
    assert.equal(attempt, 3);
    assert.equal(source.getState().saveStatus, 'saved');
  });
});

describe("saveMode: 'auto' and uploads", () => {
  it('holds the save while an upload runs and saves when it finishes', async () => {
    let finish;
    let saves = 0;
    const source = createJinshuSource({
      key: 'w3',
      initialDoc: lesson(),
      saveMode: 'auto',
      save: async () => ({ ok: true, version: `v${++saves}` }),
      media: {
        pick: async () => ({ file: { path: 'lx://temp/a.png', name: 'a.png', size: 10, type: 'image/png' } }),
        upload: () => new Promise(resolve => (finish = resolve)),
      },
    });
    const ch = fakeChannel();
    source.serve(ch, { role: 'editor' });
    ch.emit({ t: 'media', action: 'pick', kind: 'image', at: { after: 'p2' } });
    await wait(0);
    await wait(1700);
    assert.equal(saves, 0, 'no save during the upload');
    finish({ src: 'https://cdn.example/a.png' });
    await wait(1700);
    assert.equal(saves, 1);
    assert.equal(source.getState().doc.blocks[3].src, 'https://cdn.example/a.png');
  });
});

describe('one file format per document', () => {
  const MD = '# Week 3\n\nFirst **paragraph**.\n\n- one\n- two\n';

  it('json: loads the file text and saves canonical JSON with a trailing newline', async () => {
    const files = [];
    const source = createJinshuSource({
      key: 'w3',
      load: async () => ({ text: JSON.stringify(lesson()), version: 'v2' }),
      save: async (doc, base, file) => {
        files.push(file.text);
        return { ok: true, version: 'v3' };
      },
    });
    await source.load();
    assert.equal(source.format, 'json');
    assert.equal(source.getState().doc.blocks[1].id, 'p1');
    await source.save();
    assert.ok(files[0].endsWith('}\n'));
    assert.match(files[0], /\n  "blocks": \[/);
    assert.equal(JSON.parse(files[0]).blocks[1].id, 'p1');
  });

  it('json: a file that is not JSON is a problem and keeps the draft', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), load: async () => ({ text: '{ nope', version: 'v2' }) });
    await source.load();
    assert.equal(source.getState().problems[0].code, 'INVALID');
    assert.equal(source.getState().doc.blocks.length, 3);
  });

  it('markdown: reads, refuses what Markdown cannot hold, saves Markdown, and keeps ids on reload', async () => {
    const files = [];
    let stored = MD;
    const source = createJinshuSource({
      key: 'w3',
      format: 'markdown',
      initialText: MD,
      load: async () => ({ text: stored, version: 'v2' }),
      save: async (doc, base, file) => {
        files.push(file.text);
        return { ok: true, version: 'v3' };
      },
    });
    const state = source.getState();
    assert.equal(state.doc.title, 'Week 3');
    assert.deepEqual(state.doc.blocks.map(b => b.type), ['paragraph', 'list']);
    const ids = state.doc.blocks.map(b => b.id);

    const refused = source.apply({ batchId: 'v', origin: HUMAN, base: state.revision, ops: [{ op: 'insert', at: { end: 'doc' }, markup: '<details><summary>More</summary><p>x</p></details>' }] });
    assert.ok(refused.problems.some(p => p.code === 'FORMAT' && p.severity === 'error'));
    const marked = source.apply({ batchId: 'u', origin: HUMAN, base: state.revision, ops: [{ op: 'edit', id: ids[0], find: 'First', replace: '<u>First</u>' }] });
    assert.ok(marked.problems.some(p => p.code === 'FORMAT'));

    source.apply({ batchId: 'e', origin: HUMAN, base: state.revision, ops: [{ op: 'edit', id: ids[0], find: 'First', replace: 'Opening' }] });
    await source.save();
    assert.match(files[0], /^# Week 3\n/);
    assert.match(files[0], /Opening \*\*paragraph\*\*\./);

    stored = `${MD}\nA new paragraph.\n`;
    await source.load();
    const after = source.getState().doc.blocks;
    assert.equal(after.length, 3);
    assert.equal(after[1].id, ids[1], 'the unchanged list keeps its id');
  });

  it('markdown: saving keeps the bytes of blocks that did not change, across saves', async () => {
    const odd = '# Week 3\n\nFirst  *paragraph*.\n\n* one\n* two\n\nLast.\n';
    const files = [];
    const source = createJinshuSource({ key: 'w3', format: 'markdown', initialText: odd, save: async (d, b, f) => (files.push(f.text), { ok: true, version: 'v3' }) });
    const last = () => source.getState().doc.blocks.at(-1).id;
    source.apply({ batchId: 'a', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: last(), find: 'Last', replace: 'End' }] });
    await source.save();
    assert.equal(files[0], odd.replace('Last.', 'End.'));
    source.apply({ batchId: 'b', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'edit', id: last(), find: 'End', replace: 'Fin' }] });
    await source.save();
    assert.equal(files[1], odd.replace('Last.', 'Fin.'));
  });

  it('file() is what save would write, and a channel can ask for it', async () => {
    const md = '# T\n\nSoft\nwrap.\n';
    const source = createJinshuSource({ key: 'w3', format: 'markdown', initialText: md });
    assert.equal(source.file().text, md);
    const ch = fakeChannel();
    source.serve(ch, { role: 'editor' });
    ch.emit({ t: 'file' });
    assert.deepEqual(ch.sent.at(-1), { t: 'file', revision: source.getState().revision, text: md });
    const json = createJinshuSource({ key: 'j', initialDoc: lesson() });
    assert.deepEqual(JSON.parse(json.file().text).blocks[1].id, 'p1');
  });

  it('markdown: the page hears the format, media pick offers images only and drops what Markdown cannot say', async () => {
    const kinds = [];
    const source = createJinshuSource({
      key: 'w3',
      format: 'markdown',
      initialText: MD,
      media: {
        pick: async ({ kind }) => {
          kinds.push(kind);
          return { src: 'https://cdn.example/a.png', width: 640, height: 480 };
        },
      },
    });
    const ch = fakeChannel();
    source.serve(ch, { role: 'editor' });
    assert.equal(ch.sent.find(m => m.t === 'snapshot').policy.format, 'markdown');
    ch.emit({ t: 'media', action: 'pick', kind: 'video' });
    ch.emit({ t: 'media', action: 'pick', kind: 'image' });
    await wait(0);
    assert.deepEqual(kinds, ['image']);
    const img = source.getState().doc.blocks.at(-1);
    assert.equal(img.type, 'image');
    assert.equal(img.width, undefined);
    assert.equal(source.getState().problems.filter(p => p.severity === 'error').length, 0);
  });

  it('markdown: a conflict carrying the remote file text is read in the format', async () => {
    const source = createJinshuSource({
      key: 'w3',
      format: 'markdown',
      initialText: MD,
      save: async () => ({ ok: false, conflict: true, remoteText: '# Theirs\n\nTheir text.\n', remoteVersion: 'v9' }),
    });
    source.apply({ batchId: 'e', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: source.getState().doc.blocks[0].id, find: 'First', replace: 'Mine' }] });
    await source.save();
    await source.resolveConflict('theirs');
    assert.equal(source.getState().doc.title, 'Theirs');
  });

  it('jinshu_read names the format, and for Markdown says what is accepted', async () => {
    const md = createJinshuSource({ key: 'w3', format: 'markdown', initialText: MD });
    const read = await toolsOver({ w3: md }).call('jinshu_read', { doc: 'w3' });
    assert.equal(read.data.format, 'markdown');
    assert.match(read.data.formatNote, /Markdown/);
    const json = await toolsOver({ w3: createJinshuSource({ key: 'w3', initialDoc: lesson() }) }).call('jinshu_read', { doc: 'w3' });
    assert.equal(json.data.format, 'json');
    assert.equal(json.data.formatNote, undefined);
  });
});

describe('format problems as words a person reads', () => {
  it('names each lost thing once, in the reader\'s language, without the document id', async () => {
    const { audio, columns, checkMarkdownProfile, documentToMarkdown } = await import('./schema/index.ts');
    const { formatProblemWords } = await import('./source/index.ts');
    const doc = createDocument({
      id: 'doc-1',
      lang: 'zh',
      blocks: [
        paragraph([{ text: 'a', highlight: 'warning' }, { text: 'b', highlight: 'success' }], 'p'),
        audio('https://cdn.example/a.m4a', { id: 'a' }),
        columns([[paragraph('x', 'c1')], [paragraph('y', 'c2')]], { id: 'cols' }),
      ],
    });
    const problems = checkMarkdownProfile(doc);
    assert.ok(problems.some(p => p.subject && 'field' in p.subject && p.subject.field === 'id'), 'the id stays a problem');
    assert.ok(problems.every(p => p.subject), 'every FORMAT problem has a subject');
    assert.deepEqual(formatProblemWords(problems, 'zh-CN'), ['语言', '高亮', '音频', '分栏']);
    assert.deepEqual(formatProblemWords(problems, 'en'), ['Language', 'Highlight', 'Audio', 'Columns']);
    assert.match(documentToMarkdown(doc).problems[0].message, /^Markdown cannot hold/, 'agents keep the English message');
  });

  it('words what reading a Markdown file changed', async () => {
    const { markdownToDocument } = await import('./schema/index.ts');
    const { formatProblemWords } = await import('./source/index.ts');
    const read = markdownToDocument('# T\n\nA <u>b</u> c.\n\n<div>x</div>\n\n###### Deep\n');
    assert.deepEqual(formatProblemWords(read.problems, 'zh'), ['HTML 保留为文字', '五、六级标题改为四级']);
  });
});
