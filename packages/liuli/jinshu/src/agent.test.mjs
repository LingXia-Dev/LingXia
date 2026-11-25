import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createDocument, paragraph, heading, plainText } from './schema/index.ts';
import { createJinshuSource, createJinshuSources, createJinshuTools, bindJinshuPage } from './source/index.ts';
import { agentCall, toolsOver } from './testing/agent.mjs';

const lesson = () =>
  createDocument({
    title: 'Week 3',
    blocks: [heading(1, 'Week 3', 'h'), paragraph('First paragraph.', 'p1'), paragraph('Second paragraph.', 'p2'), paragraph('Third.', 'p3')],
  });

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

describe('Agent tools: effects per call', () => {
  const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), agentSave: 'confirm' });
  const t = toolsOver({ w3: source });

  it('names the five tools with a summary, a description and a schema each', () => {
    assert.deepEqual(t.tools.map(x => x.name), ['jinshu_list', 'jinshu_read', 'jinshu_edit', 'jinshu_undo', 'jinshu_save']);
    for (const tool of t.tools) {
      assert.match(tool.name, /^[a-z][a-z0-9_]{0,63}$/);
      assert.ok(tool.summary && tool.description && tool.inputSchema.type === 'object');
    }
  });

  it('list and read are reads', async () => {
    assert.equal((await t.assess('jinshu_list', {})).effect, 'read');
    assert.equal((await t.assess('jinshu_read', { doc: 'w3' })).effect, 'read');
  });

  it('a suggest edit is a read; a direct edit and an undo are writes, summarised in the caller language', async () => {
    const ops = [{ op: 'delete', id: 'p3' }];
    assert.equal((await t.assess('jinshu_edit', { doc: 'w3', base: 1, mode: 'suggest', ops })).effect, 'read');
    const direct = await t.assess('jinshu_edit', { doc: 'w3', base: 1, ops });
    assert.equal(direct.effect, 'write');
    assert.equal(direct.summary, '“Week 3”: 1 change');
    const zh = await t.assess('jinshu_edit', { doc: 'w3', base: 1, ops }, agentCall({ language: 'zh-CN' }));
    assert.equal(zh.summary, '《Week 3》：1 处修改');
    assert.equal((await t.assess('jinshu_undo', { doc: 'w3' })).effect, 'write');
  });

  it('save follows agentSave', async () => {
    const effectFor = async agentSave => {
      const s = createJinshuSource({ key: 'k', initialDoc: lesson(), agentSave });
      return (await toolsOver({ k: s }).assess('jinshu_save', { doc: 'k' })).effect;
    };
    assert.equal(await effectFor('allow'), 'write');
    assert.equal(await effectFor('confirm'), 'read');
    assert.equal(await effectFor('deny'), 'read');
    const denied = createJinshuSource({ key: 'k', initialDoc: lesson(), agentSave: 'deny' });
    assert.equal((await toolsOver({ k: denied }).call('jinshu_save', { doc: 'k' })).status, 'error');
  });
});

describe('Agent tools: results', () => {
  it('a suggestion and a save request are pending-review with the page', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), agentSave: 'confirm' });
    const t = toolsOver({ w3: source });
    const sug = await t.call('jinshu_edit', { doc: 'w3', base: 1, mode: 'suggest', ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Opening' }] });
    assert.equal(sug.status, 'pending-review');
    assert.equal(sug.page, 'pages/doc?key=w3');
    assert.ok(sug.data.suggestionId);
    assert.match(sug.summary, /Week 3/);
    source.rejectSuggestion(sug.data.suggestionId);
    const save = await t.call('jinshu_save', { doc: 'w3', summary: 'Tidy up' });
    assert.equal(save.status, 'pending-review');
    assert.equal(save.page, 'pages/doc?key=w3');
  });

  it('a large direct edit is held as a suggestion, said why', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const res = await toolsOver({ w3: source }).call('jinshu_edit', {
      doc: 'w3',
      base: 1,
      ops: [{ op: 'delete', id: 'p1' }, { op: 'delete', id: 'p2' }],
    });
    assert.equal(res.status, 'pending-review');
    assert.equal(res.data.downgraded, 'large');
    assert.equal(source.getState().suggestions[0].downgraded, 'large');
  });

  it('takes the origin from the call, never the arguments', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const res = await toolsOver({ w3: source }).call('jinshu_edit', {
      doc: 'w3',
      base: 1,
      origin: { kind: 'human', id: 'forged', label: 'Admin' },
      ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Next' }],
    }, agentCall({ agent: 'Kimi', conversation: 'c9' }));
    assert.equal(res.status, 'ok');
    const entry = source.getState().history.at(-1);
    assert.deepEqual(entry.origin, { kind: 'agent', id: 'dazi/c9', label: 'Kimi', conversation: 'c9', via: 'Dazi' });
  });

  it('undo reaches only the calling conversation', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const t = toolsOver({ w3: source });
    await t.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Next' }] });
    const other = await t.call('jinshu_undo', { doc: 'w3' }, agentCall({ conversation: 'other' }));
    assert.equal(other.status, 'error');
    const own = await t.call('jinshu_undo', { doc: 'w3' });
    assert.equal(own.status, 'ok');
    assert.equal(plainText(source.getState().doc.blocks[2].runs), 'Second paragraph.');
  });

  it('list takes a query, caps rows at 20 and flags on-screen, dirty and waiting', async () => {
    const sources = {};
    for (let i = 0; i < 25; i++) sources[`d${i}`] = createJinshuSource({ key: `d${i}`, initialDoc: createDocument({ title: `Doc ${i}`, blocks: [paragraph('x', 'p')] }) });
    const t = toolsOver(sources);
    const all = await t.call('jinshu_list', {});
    assert.equal(all.data.documents.length, 20);
    assert.equal(all.data.more, true);
    sources.d7.serve(fakeChannel());
    sources.d7.apply({ batchId: 'b', origin: { kind: 'human', id: 'me' }, base: 1, ops: [{ op: 'edit', id: 'p', find: 'x', replace: 'y' }] });
    sources.d7.apply({ batchId: 'c', origin: { kind: 'agent', id: 'a' }, base: 2, mode: 'suggest', ops: [{ op: 'edit', id: 'p', find: 'y', replace: 'z' }] });
    const one = await t.call('jinshu_list', { query: 'Doc 7' });
    assert.deepEqual(one.data.documents, [{ key: 'd7', title: 'Doc 7', onScreen: true, dirty: true, waiting: 1, page: 'pages/doc?key=d7' }]);
  });
});

describe('Agent tools: jinshu_read waitMs', () => {
  it('returns at once when something changed after since', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    source.apply({ batchId: 'b', origin: { kind: 'human', id: 'me' }, base: 1, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Top' }] });
    const started = performance.now();
    const res = await toolsOver({ w3: source }).call('jinshu_read', { doc: 'w3', since: 1, waitMs: 20_000 });
    assert.ok(performance.now() - started < 1000);
    assert.equal(res.data.revision, 2);
  });

  it('resolves on a review outcome while waiting, with the outcome', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const t = toolsOver({ w3: source });
    const sug = await t.call('jinshu_edit', { doc: 'w3', base: 1, mode: 'suggest', ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Opening' }] });
    const since = source.getState().revision;
    const pending = t.call('jinshu_read', { doc: 'w3', since, waitMs: 20_000 });
    const started = performance.now();
    await Promise.resolve();
    source.acceptSuggestion(sug.data.suggestionId);
    const res = await pending;
    assert.ok(performance.now() - started < 1000);
    assert.deepEqual(res.data.reviews.map(r => [r.kind, r.accepted]), [['suggestion', true]]);
    assert.deepEqual(res.data.pending, []);
  });

  it('gives up after waitMs, and the caller can abort sooner', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const t = toolsOver({ w3: source });
    const started = performance.now();
    const res = await t.call('jinshu_read', { doc: 'w3', since: 1, waitMs: 60 });
    assert.ok(performance.now() - started >= 50);
    assert.deepEqual(res.data.reviews, []);
    const controller = new AbortController();
    const aborted = t.call('jinshu_read', { doc: 'w3', since: 1, waitMs: 25_000 }, agentCall({ signal: controller.signal }));
    controller.abort();
    assert.equal((await aborted).status, 'ok');
  });

  it('caps waitMs at 25 s in its schema', () => {
    const read = toolsOver({}).tools.find(x => x.name === 'jinshu_read');
    assert.equal(read.inputSchema.properties.waitMs.maximum, 25_000);
  });

  it('reports a save decision and a stop as reviews', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson(), agentSave: 'confirm' });
    const t = toolsOver({ w3: source });
    await t.call('jinshu_save', { doc: 'w3' });
    const since = source.getState().revision;
    source.cancelAgentSave();
    const res = await t.call('jinshu_read', { doc: 'w3', since });
    assert.deepEqual(res.data.reviews.map(r => [r.kind, r.accepted]), [['save', false]]);
    assert.equal(source.getState().dirty, false);
  });

  it('wakes a waiting read when the person takes back a direct change, and reports it with the batch', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const t = toolsOver({ w3: source });
    const edit = await t.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Next' }] });
    const since = source.getState().revision;
    const pending = t.call('jinshu_read', { doc: 'w3', since, waitMs: 20_000 });
    await Promise.resolve();
    source.undo(edit.data.batchId);
    const res = await pending;
    assert.deepEqual(res.data.reviews.map(r => [r.kind, r.batchId, r.blockIds]), [['undone', edit.data.batchId, ['p2']]]);
    source.redo();
    const after = await t.call('jinshu_read', { doc: 'w3', since: res.data.revision });
    assert.deepEqual(after.data.reviews.map(r => r.kind), ['redone']);
  });

  it('does not report an agent undoing its own change', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const t = toolsOver({ w3: source });
    await t.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Next' }] });
    const since = source.getState().revision;
    await t.call('jinshu_undo', { doc: 'w3' });
    assert.deepEqual((await t.call('jinshu_read', { doc: 'w3', since })).data.reviews, []);
  });
});

describe('Shared sources', () => {
  it('opens one source per key, shared by a page and the tools', async () => {
    let loads = 0;
    const sources = createJinshuSources(key =>
      key === 'w3' ? { load: async () => (loads++, { doc: lesson(), version: 'v1' }) } : null,
    );
    const page = await sources.open('w3');
    const tools = createJinshuTools({ documents: { list: () => [], open: key => sources.open(key), peek: key => sources.peek(key) } });
    const edit = tools.find(x => x.name === 'jinshu_edit');
    const res = await edit.run({ doc: 'w3', base: page.getState().revision, ops: [{ op: 'edit', id: 'p1', find: 'First', replace: 'Agent' }] }, agentCall());
    assert.equal(res.status, 'ok');
    assert.equal(await sources.open('w3'), page);
    assert.equal(loads, 1);
    assert.equal(plainText(page.getState().doc.blocks[1].runs), 'Agent paragraph.');
    assert.equal(await sources.open('nope'), null);
    assert.equal(sources.peek('nope'), undefined);
  });

  it('a page leaving keeps the source and its draft; the next page sees what changed while away', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const updates = [];
    let unload;
    const { syncDoc } = bindJinshuPage({ update: p => updates.push(p), onUnload: fn => (unload = fn) }, source);
    const first = fakeChannel();
    syncDoc(undefined, first);
    assert.equal(source.getState().viewers, 1);
    first.close();
    unload();
    assert.equal(source.getState().viewers, 0);
    const res = await toolsOver({ w3: source }).call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Later' }] });
    assert.equal(res.status, 'ok');
    const next = fakeChannel();
    source.serve(next, { doc: 'w3', role: 'editor' });
    const away = next.sent.find(m => m.t === 'agentBatches');
    assert.equal(away.batches[0].away, true);
    assert.deepEqual(away.batches[0].blockIds, ['p2']);
    assert.equal(plainText(source.getState().doc.blocks[2].runs), 'Later paragraph.');
  });

  it('the person stops an agent: its next edit is refused until it reads again', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const t = toolsOver({ w3: source });
    const ch = fakeChannel();
    source.serve(ch);
    await t.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'edit', id: 'p2', find: 'Second', replace: 'Next' }] });
    const writing = source.getState().activities[0];
    assert.equal(writing.kind, 'writing');
    ch.emit({ t: 'stopAgent', originId: writing.origin.id });
    const refused = await t.call('jinshu_edit', { doc: 'w3', base: source.getState().revision, ops: [{ op: 'delete', id: 'p3' }] });
    assert.equal(refused.code, 'STOPPED');
    const read = await t.call('jinshu_read', { doc: 'w3' });
    assert.equal(read.data.stopped, true);
    const again = await t.call('jinshu_edit', { doc: 'w3', base: source.getState().revision, ops: [{ op: 'delete', id: 'p3' }] });
    assert.equal(again.status, 'ok');
  });
});

describe('what an agent insert touched', () => {
  it('names the added blocks, not the anchor, so the mark and an undo point at the new text', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const tools = toolsOver({ w3: source });
    const edit = await tools.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'insert', at: { after: 'p1' }, markup: '<p>Added.</p>' }] });
    const added = source.getState().doc.blocks[2].id;
    const since = source.getState().revision;
    source.undo(edit.data.batchId);
    const read = await tools.call('jinshu_read', { doc: 'w3', since, outline: true });
    const undone = read.data.reviews.find(r => r.kind === 'undone');
    assert.deepEqual(undone.blockIds, [added]);
  });
});

describe('what an agent insert touched, nested', () => {
  it('counts a new callout as one change, not one per paragraph inside it', async () => {
    const source = createJinshuSource({ key: 'w3', initialDoc: lesson() });
    const tools = toolsOver({ w3: source });
    const edit = await tools.call('jinshu_edit', { doc: 'w3', base: 1, ops: [{ op: 'insert', at: { after: 'p1' }, markup: '<callout tone="info"><p>One.</p><p>Two.</p></callout>' }] });
    const callout = source.getState().doc.blocks[2].id;
    const since = source.getState().revision;
    source.undo(edit.data.batchId);
    const read = await tools.call('jinshu_read', { doc: 'w3', since, outline: true });
    assert.deepEqual(read.data.reviews.find(r => r.kind === 'undone').blockIds, [callout]);
  });
});
