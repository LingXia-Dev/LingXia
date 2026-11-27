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

describe('changed while away: the editor takes it with undo, a reader is told once', () => {
  it('a reader opening first is told, and still leaves the away batches for the editor', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    await agentEdit(source, 'p1', 'First', 'Away');
    const reader = fakeChannel();
    source.serve(reader, { doc: 'w3', role: 'reader' });
    const [told] = batchesIn(reader);
    assert.equal(told.away, true);
    assert.deepEqual(told.blockIds, ['p1']);
    // Once: the next reader does not hear it again.
    const nextReader = fakeChannel();
    source.serve(nextReader, { doc: 'w3', role: 'reader' });
    assert.equal(batchesIn(nextReader).length, 0);
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

  it('a change while only a reader shows the document goes to it live, and waits for an editor, not marked away', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const reader = fakeChannel();
    source.serve(reader, { role: 'reader' });
    await agentEdit(source, 'p2', 'Second', 'Later');
    const [live] = batchesIn(reader);
    assert.deepEqual(live.blockIds, ['p2']);
    assert.equal(live.away, undefined);
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

  it('a reader is not told of a change undone meanwhile, nor of one an editor showed first', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const undone = await agentEdit(source, 'p1', 'First', 'Gone');
    source.undo(undone.data.batchId);
    await agentEdit(source, 'p2', 'Second', 'Kept');
    const reader = fakeChannel();
    source.serve(reader, { role: 'reader' });
    assert.deepEqual(batchesIn(reader).map(b => b.blockIds[0]), ['p2']);
    reader.close();

    await agentEdit(source, 'p2', 'Kept', 'Again');
    source.serve(fakeChannel(), { role: 'editor' });
    const late = fakeChannel();
    source.serve(late, { role: 'reader' });
    assert.equal(batchesIn(late).length, 0);
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

describe('stale batches', () => {
  const replaceP1 = (base, text) => ({ t: 'ops', batch: { batchId: `b${text}`, origin: HUMAN, base, mode: 'direct', ops: [{ op: 'replace', id: 'p1', blocks: [paragraph(text, 'p1')] }] } });
  const insertAfterP1 = base => ({ t: 'ops', batch: { batchId: 'bi', origin: HUMAN, base, mode: 'direct', ops: [{ op: 'insert', at: { after: 'p1' }, blocks: [paragraph('Next', 'n')] }] } });

  it("an editor's own earlier batch never makes its next one stale", () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const editor = fakeChannel();
    source.serve(editor, { doc: 'w3', role: 'editor' });
    const base = source.getState().revision;
    editor.emit(replaceP1(base, 'Saved'));
    editor.emit(insertAfterP1(base));
    assert.deepEqual(editor.sent.filter(m => m.t === 'reject'), []);
    assert.deepEqual(source.getState().doc.blocks.map(b => b.id), ['h', 'p1', 'n', 'p2']);
  });

  it("another editor's change to the block still does", () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const one = fakeChannel();
    const two = fakeChannel();
    source.serve(one, { doc: 'w3', role: 'editor' });
    source.serve(two, { doc: 'w3', role: 'editor' });
    const base = source.getState().revision;
    one.emit(replaceP1(base, 'Mine'));
    two.emit(insertAfterP1(base));
    assert.equal(two.sent.filter(m => m.t === 'reject')[0]?.problems[0].code, 'STALE_BLOCK');
  });
});

describe('history replays what happened', () => {
  const AGENT = { kind: 'agent', id: 'codex', label: 'Codex' };

  it('undoing an earlier unrelated batch keeps the ids later batches minted and built on', () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), guardLargeAgentEdits: false });
    const rev = () => source.getState().revision;
    source.apply({ batchId: 'h1', origin: HUMAN, base: rev(), ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Opening' }] });
    source.apply({ batchId: 'a1', origin: AGENT, base: rev(), ops: [{ op: 'insert', at: { after: 'p2' }, markup: '<p>Agent line.</p>' }] });
    const added = source.getState().doc.blocks.at(-1).id;
    source.apply({ batchId: 'h2', origin: HUMAN, base: rev(), ops: [{ op: 'edit', id: added, find: 'Agent', replace: 'Kept' }] });
    const res = source.undo('h1');
    assert.deepEqual(res.problems, []);
    const blocks = source.getState().doc.blocks;
    assert.equal(blocks.at(-1).id, added);
    assert.equal(plainText(blocks.at(-1).runs), 'Kept line.');
    assert.equal(plainText(blocks[1].runs), 'First paragraph.');
  });

  it('redo restores the same ids, rows included', () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    source.apply({ batchId: 'i', origin: HUMAN, base: 1, ops: [{ op: 'insert', at: { after: 'p2' }, markup: '<table id="t"><tr><td>a</td></tr></table>' }] });
    source.apply({ batchId: 'r', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'tableRow', id: 't', action: 'insert' }] });
    const before = source.getState().doc;
    source.undo();
    source.undo();
    source.redo();
    source.redo();
    assert.deepEqual(source.getState().doc, before);
  });
});

describe('an upload finishing never brings back the local file', () => {
  const uploading = () => {
    let finish;
    const source = createJinshuSource({
      key: 'w3',
      initialDoc: lesson(),
      media: {
        pick: async () => ({ file: { path: 'lx://temp/a.png', name: 'a.png', size: 10, type: 'image/png' } }),
        upload: () => new Promise(resolve => (finish = resolve)),
      },
    });
    const ch = fakeChannel();
    source.serve(ch, { role: 'editor' });
    return { source, ch, done: src => finish({ src, width: 4, height: 3 }) };
  };
  const srcs = source => source.getState().doc.blocks.filter(b => b.type === 'image').map(b => b.src);

  it('undo then redo restores the uploaded src', async () => {
    const { source, ch, done } = uploading();
    ch.emit({ t: 'media', action: 'pick', kind: 'image', at: { after: 'p2' } });
    await wait(0);
    done('https://cdn.example/a.png');
    await wait(0);
    source.undo();
    source.redo();
    assert.deepEqual(srcs(source), ['https://cdn.example/a.png']);
  });

  it('undoing an earlier batch replays the uploaded src', async () => {
    const { source, ch, done } = uploading();
    source.apply({ batchId: 'e', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Opening' }] });
    ch.emit({ t: 'media', action: 'pick', kind: 'image', at: { after: 'p2' } });
    await wait(0);
    done('https://cdn.example/a.png');
    await wait(0);
    assert.deepEqual(source.undo('e').problems, []);
    assert.deepEqual(srcs(source), ['https://cdn.example/a.png']);
  });

  it('a replacement undone while uploading stays undone, and redo brings the upload', async () => {
    const { source, ch, done } = uploading();
    source.apply({ batchId: 'i', origin: HUMAN, base: 1, ops: [{ op: 'insert', at: { after: 'p2' }, blocks: [{ id: 'img', type: 'image', src: 'https://cdn.example/old.png' }] }] });
    ch.emit({ t: 'media', action: 'pick', kind: 'image', replacingId: 'img' });
    await wait(0);
    source.undo();
    done('https://cdn.example/new.png');
    await wait(0);
    assert.deepEqual(srcs(source), ['https://cdn.example/old.png']);
    source.redo();
    assert.deepEqual(srcs(source), ['https://cdn.example/new.png']);
  });
});

describe('replacing the draft starts a new history', () => {
  const MD = '# Week 3\n\nFirst **paragraph**.\n\n- one\n- two\n';
  it("undo after 'theirs' does not rewind to the draft it dropped", async () => {
    const theirs = createDocument({ title: 'Theirs', blocks: [paragraph('Their text.', 'p1')] });
    const source = createJinshuSource({
      key: 'w3',
      initialDoc: lesson(),
      save: async () => ({ ok: false, conflict: true, remoteDoc: theirs, remoteVersion: 'v9' }),
    });
    source.apply({ batchId: 'e', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Mine' }] });
    await source.save();
    await source.resolveConflict('theirs');
    assert.equal(source.getState().canUndo, false);
    assert.equal(source.undo().problems[0].code, 'NOT_FOUND');
    assert.equal(source.getState().doc.title, 'Theirs');
  });

  it('a batch based before a load is stale', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), load: async () => ({ doc: lesson(), version: 'v2' }) });
    const base = source.getState().revision;
    await source.load();
    const res = source.apply({ batchId: 'x', origin: HUMAN, base, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Late' }] });
    assert.equal(res.problems[0]?.code, 'STALE_BLOCK');
  });

  it("markdown: 'theirs' keeps the remote file's bytes for what did not change", async () => {
    const remote = '# Theirs\n\nTheir __text__.\n\n* one\n* two\n';
    const source = createJinshuSource({
      key: 'w3',
      format: 'markdown',
      initialText: MD,
      save: async () => ({ ok: false, conflict: true, remoteText: remote, remoteVersion: 'v9' }),
    });
    source.apply({ batchId: 'e', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: source.getState().doc.blocks[0].id, find: 'First', replace: 'Mine' }] });
    await source.save();
    await source.resolveConflict('theirs');
    assert.equal(source.file().text, remote);
  });

  it('markdown: a restored draft keeps the bytes of the file it was based on', async () => {
    const odd = '# Week 3\n\nFirst __paragraph__.\n\n* one\n\nLast.\n';
    const store = new Map();
    const storage = { get: async k => store.get(k), set: async (k, v) => void store.set(k, v), remove: async k => void store.delete(k) };
    const first = createJinshuSource({ key: 'w3', format: 'markdown', initialText: odd, storage });
    const last = first.getState().doc.blocks.at(-1).id;
    first.apply({ batchId: 'a', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: last, find: 'Last', replace: 'End' }] });
    await wait(0);
    const second = createJinshuSource({ key: 'w3', format: 'markdown', storage });
    assert.equal(await second.restoreDraft(), true);
    assert.equal(second.file().text, odd.replace('Last.', 'End.'));
    assert.equal(second.getState().canUndo, false);
  });
});

describe('save state reaches the page over the channel', () => {
  it('sends dirty and saving as they change', async () => {
    let finish;
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), save: () => new Promise(r => (finish = r)) });
    const ch = fakeChannel();
    source.serve(ch, { role: 'editor' });
    const states = () => ch.sent.filter(m => m.t === 'saveState');
    assert.deepEqual(states().at(-1), { t: 'saveState', dirty: false, saveStatus: 'saved' });
    ch.emit({ t: 'ops', batch: { batchId: 'b', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'New' }] } });
    assert.deepEqual(states().at(-1), { t: 'saveState', dirty: true, saveStatus: 'dirty' });
    ch.emit({ t: 'save', action: 'save', requestId: 'q1' });
    await wait(0);
    assert.equal(states().at(-1).saveStatus, 'saving');
    finish({ ok: true, version: 'v2' });
    await wait(0);
    assert.deepEqual(states().at(-1), { t: 'saveState', dirty: false, saveStatus: 'saved' });
    assert.deepEqual(ch.sent.find(m => m.t === 'saveResult'), { t: 'saveResult', requestId: 'q1', ok: true });
    const count = states().length;
    source.setFocus(['p1']);
    assert.equal(states().length, count, 'unchanged state is not resent');
  });

  it('a save that meets a conflict answers not ok', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), save: async () => ({ ok: false, conflict: true, remoteDoc: lesson(), remoteVersion: 'v9' }) });
    const ch = fakeChannel();
    source.serve(ch, { role: 'editor' });
    ch.emit({ t: 'ops', batch: { batchId: 'b', origin: HUMAN, base: 1, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'New' }] } });
    ch.emit({ t: 'save', action: 'save', requestId: 'q1' });
    await wait(0);
    assert.deepEqual(ch.sent.find(m => m.t === 'saveResult'), { t: 'saveResult', requestId: 'q1', ok: false });
  });
});

describe('focus belongs to each editor', () => {
  const AGENT = { kind: 'agent', id: 'codex' };
  const agentOp = (source, id) => source.apply({ batchId: `a${id}${source.getState().revision}`, origin: AGENT, base: source.getState().revision, ops: [{ op: 'edit', id, find: 'paragraph', replace: 'para' }] });

  it("one editor blurring keeps another's focus, and a closed editor's focus goes", () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const one = fakeChannel();
    const two = fakeChannel();
    source.serve(one, { role: 'editor' });
    source.serve(two, { role: 'editor' });
    one.emit({ t: 'focus', blockIds: ['p1'] });
    two.emit({ t: 'focus', blockIds: ['p2'] });
    two.emit({ t: 'focus', blockIds: [] });
    assert.deepEqual(source.getState().focusIds, ['p1']);
    assert.ok(agentOp(source, 'p1').suggestionId, 'still downgraded on the focused block');
    one.close();
    assert.deepEqual(source.getState().focusIds, []);
    assert.equal(agentOp(source, 'p1').suggestionId, undefined);
  });
});

describe('a draft edited back to what was saved is clean', () => {
  it('undo to the saved content clears dirty; redo makes it dirty again', () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    source.apply({ batchId: 'e', origin: HUMAN, base: 1, ops: [{ op: 'text', target: { blockId: 'p1' }, from: 0, to: 5, insert: [{ text: 'Only' }] }] });
    assert.equal(source.getState().dirty, true);
    source.undo();
    assert.equal(source.getState().dirty, false);
    assert.equal(source.getState().saveStatus, 'saved');
    source.redo();
    assert.equal(source.getState().dirty, true);
  });

  it('retyping the saved text by hand is clean too', () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    source.apply({ batchId: 'a', origin: HUMAN, base: 1, ops: [{ op: 'text', target: { blockId: 'p1' }, from: 0, to: 5, insert: [{ text: 'Only' }] }] });
    source.apply({ batchId: 'b', origin: HUMAN, base: source.getState().revision, ops: [{ op: 'text', target: { blockId: 'p1' }, from: 0, to: 4, insert: [{ text: 'First' }] }] });
    assert.equal(source.getState().dirty, false);
  });
});
