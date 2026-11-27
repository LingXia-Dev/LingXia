// Property tests: random concurrent human + agent op sequences against createJinshuSource, seeded so a
// failure names the seed and step that reproduce it.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { checkDocument, createDocument, heading, listBlocks, paragraph, plainText } from './schema/index.ts';
import { createJinshuSource } from './source/index.ts';

const SEEDS = Number(process.env.JINSHU_PROPERTY_SEEDS ?? 60);
const STEPS = Number(process.env.JINSHU_PROPERTY_STEPS ?? 120);
const ID_RE = /^[a-z0-9_-]{1,32}$/;

function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Runs `fn` with Math.random and Date.now under the test's control, so ids and coalescing replay exactly. */
function controlled(seed, fn) {
  const random = Math.random;
  const now = Date.now;
  const rand = mulberry32(seed ^ 0x9e3779b9);
  let clock = 1_700_000_000_000;
  Math.random = rand;
  Date.now = () => clock;
  try {
    return fn({ tick: ms => (clock += ms) });
  } finally {
    Math.random = random;
    Date.now = now;
  }
}

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
  };
}

const HUMAN = { kind: 'human', id: 'editor' };
const AGENTS = [
  { kind: 'agent', id: 'agent-a', label: 'A' },
  { kind: 'agent', id: 'agent-b', label: 'B' },
];
const TOKEN_RE = /[HA]\d+x/g;
const textOf = block => plainText(block.runs ?? []);
const tokensIn = doc => new Set(listBlocks(doc).flatMap(b => textOf(b).match(TOKEN_RE) ?? []));
const idsIn = doc => {
  const ids = [];
  for (const b of listBlocks(doc)) {
    ids.push(b.id);
    if (b.type === 'list') for (const item of b.items) ids.push(item.id);
  }
  return ids;
};
const errorsIn = problems => problems.filter(p => p.severity === 'error');
const clone = value => JSON.parse(JSON.stringify(value));
const escapeXml = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** Offsets in a paragraph where a word starts or ends, so tokens are never cut. */
function wordEdges(text) {
  const edges = [0, text.length];
  for (let i = 0; i < text.length; i++) if (text[i] === ' ') edges.push(i);
  return [...new Set(edges)];
}

function runSequence(seed) {
  return controlled(seed, ({ tick }) => {
    const rand = mulberry32(seed);
    const pick = list => list[Math.floor(rand() * list.length)];
    const chance = p => rand() < p;

    const initial = createDocument({
      title: 'Property',
      blocks: [heading(1, 'Start', 'h0'), ...['alpha', 'beta', 'gamma', 'delta'].map((w, i) => paragraph(`${w} one two`, `p${i}`))],
    });
    const source = createJinshuSource({ key: `prop-${seed}`, initialDoc: initial });
    const editor = fakeChannel();
    source.serve(editor, { doc: source.key, role: 'editor' });

    let counter = 0;
    let freshIds = 0;
    const fresh = prefix => `${prefix}${seed.toString(36)}-${freshIds++}`;
    const everSeen = new Set(idsIn(initial));
    // When each human token last appeared, so an agent removing one it never saw is caught.
    const tokenSeenAt = new Map();
    const agents = AGENTS.map(origin => ({ origin, base: 1, doc: clone(initial), batches: [] }));
    let focused = null; // what the editor has reported
    let pendingFocus = null; // what the person is editing, reported `delay` steps late
    let humanTokens = new Set();
    const log = [];

    // Read through `subscribe` during a step; each step ends with a real getState() to check it.
    let latest = source.getState();
    source.subscribe(s => (latest = s));
    const state = () => latest;
    const doc = () => state().doc;
    const paragraphs = () => doc().blocks.filter(b => b.type === 'paragraph');
    const fail = (step, message) => assert.fail(`seed ${seed} step ${step}: ${message}\n${log.slice(-8).join('\n')}`);

    function human(ops) {
      const batchId = `hb-${counter++}`;
      editor.emit({ t: 'ops', batch: { batchId, origin: HUMAN, base: state().revision, ops } });
      const reply = editor.sent.findLast(m => (m.t === 'ack' || m.t === 'reject') && m.batchId === batchId);
      editor.sent.length = 0;
      return reply;
    }

    for (let step = 0; step < STEPS; step++) {
      tick(chance(0.15) ? 100 : 1_000); // now and then within 500 ms: coalesced human typing
      const before = clone(doc());
      const beforeTokens = tokensIn(before);
      const beforeRev = state().revision;
      if (pendingFocus && --pendingFocus.delay <= 0) {
        editor.emit({ t: 'focus', blockIds: [pendingFocus.id] });
        focused = pendingFocus.id;
        pendingFocus = null;
      }

      const roll = rand();
      let action = '';
      if (roll < 0.3) {
        // The person types a token into the block they are editing (focus may not have reached the source yet).
        const target = paragraphs().find(b => b.id === (pendingFocus?.id ?? focused)) ?? pick(paragraphs());
        if (!target) continue;
        if (target.id !== focused && !pendingFocus) {
          if (chance(0.5)) pendingFocus = { id: target.id, delay: 1 + Math.floor(rand() * 3) };
          else {
            editor.emit({ t: 'focus', blockIds: [target.id] });
            focused = target.id;
          }
        }
        const token = `H${counter++}x`;
        const at = pick(wordEdges(textOf(target)));
        action = `human types ${token} in ${target.id}@${at}`;
        const reply = human([{ op: 'text', target: { blockId: target.id }, from: at, to: at, insert: [{ text: ` ${token} ` }] }]);
        if (reply?.t !== 'ack') fail(step, `${action} refused: ${JSON.stringify(reply?.problems)}`);
        if (!tokensIn(doc()).has(token)) fail(step, `${action}: the token is not in the document`);
        humanTokens.add(token);
      } else if (roll < 0.36) {
        const target = pick(paragraphs());
        if (!target) continue;
        const newId = fresh('s');
        const at = pick(wordEdges(textOf(target)));
        action = `human splits ${target.id}@${at} -> ${newId}`;
        const reply = human([{ op: 'split', id: target.id, at, newId }]);
        if (reply?.t !== 'ack') fail(step, `${action} refused: ${JSON.stringify(reply?.problems)}`);
        const blocks = doc().blocks;
        const i = blocks.findIndex(b => b.id === target.id);
        if (i < 0 || blocks[i + 1]?.id !== newId) fail(step, `${action}: the first half keeps the id, the second takes ${newId}`);
        everSeen.add(newId);
      } else if (roll < 0.41) {
        const blocks = doc().blocks;
        const i = blocks.findIndex((b, k) => k > 0 && b.type === 'paragraph' && blocks[k - 1].type === 'paragraph');
        if (i < 0) continue;
        const [prev, curr] = [blocks[i - 1], blocks[i]];
        action = `human merges ${curr.id} into ${prev.id}`;
        const reply = human([{ op: 'merge', id: curr.id }]);
        if (reply?.t !== 'ack') fail(step, `${action} refused: ${JSON.stringify(reply?.problems)}`);
        if (!doc().blocks.some(b => b.id === prev.id) || doc().blocks.some(b => b.id === curr.id)) fail(step, `${action}: the earlier block keeps its id`);
        if (focused === curr.id) focused = null;
      } else if (roll < 0.46) {
        const anchor = pick(doc().blocks);
        const id = fresh('n');
        action = `human inserts ${id} after ${anchor.id}`;
        const reply = human([{ op: 'insert', at: { after: anchor.id }, blocks: [{ id, type: 'paragraph', runs: [{ text: 'new words' }] }] }]);
        if (reply?.t !== 'ack') fail(step, `${action} refused: ${JSON.stringify(reply?.problems)}`);
        everSeen.add(id);
      } else if (roll < 0.49) {
        const target = paragraphs().find(b => b.id !== focused);
        if (!target || doc().blocks.length < 3) continue;
        action = `human deletes ${target.id}`;
        const reply = human([{ op: 'delete', id: target.id }]);
        if (reply?.t !== 'ack') fail(step, `${action} refused: ${JSON.stringify(reply?.problems)}`);
      } else if (roll < 0.56) {
        // Undo, then redo, is the identity; a plain undo is left in place half the time.
        const res = source.undo();
        action = `human undo (${res.problems[0]?.code ?? 'ok'})`;
        if (errorsIn(res.problems).length === 0) {
          const undone = clone(doc());
          if (chance(0.5)) {
            const redo = source.redo();
            action += `, redo (${redo.problems[0]?.code ?? 'ok'})`;
            if (errorsIn(redo.problems).length) fail(step, `${action}: redo right after undo refused ${JSON.stringify(redo.problems)}`);
            assert.deepEqual(doc(), before, `seed ${seed} step ${step}: undo then redo is not the identity\n${log.slice(-8).join('\n')}`);
          } else {
            assert.notDeepEqual(undone, before, `seed ${seed} step ${step}: an undo changed nothing`);
          }
        }
      } else if (roll < 0.6) {
        const res = source.redo();
        action = `human redo (${res.problems[0]?.code ?? 'ok'})`;
      } else if (roll < 0.66) {
        const sug = pick(state().suggestions);
        if (!sug) continue;
        const accept = chance(0.6);
        const res = accept ? source.acceptSuggestion(sug.id, HUMAN) : source.rejectSuggestion(sug.id);
        action = `human ${accept ? 'accepts' : 'rejects'} ${sug.kind} suggestion ${sug.id} (${res.problems[0]?.code ?? 'ok'})`;
        if (!accept && errorsIn(res.problems).length) fail(step, `${action}: a reject never fails`);
      } else if (roll < 0.68) {
        editor.emit({ t: 'focus', blockIds: [] });
        focused = null;
        pendingFocus = null;
        action = 'human blurs';
      } else {
        // An agent reads, or edits from what it last read.
        const agent = pick(agents);
        if (chance(0.25)) {
          agent.base = state().revision;
          agent.doc = clone(doc());
          action = `${agent.origin.id} reads @${agent.base}`;
        } else if (chance(0.12) && agent.batches.length) {
          const batchId = agent.batches.pop();
          const res = source.undo(batchId, agent.origin);
          action = `${agent.origin.id} undoes ${batchId} (${res.problems[0]?.code ?? 'ok'})`;
          if (errorsIn(res.problems).length === 0) {
            const after = tokensIn(doc());
            for (const t of beforeTokens) if (humanTokens.has(t) && !after.has(t)) fail(step, `${action} removed the person's ${t}`);
          }
        } else {
          if (chance(0.35)) {
            agent.base = state().revision;
            agent.doc = clone(doc());
          }
          const seenBlocks = agent.doc.blocks.filter(b => b.type === 'paragraph');
          // Often the block the person is in, so focus turns edits into suggestions.
          const target = (chance(0.4) && seenBlocks.find(b => b.id === (pendingFocus?.id ?? focused))) || pick(seenBlocks);
          if (!target) continue;
          const token = `A${counter++}x`;
          const kind = pick(['edit', 'edit', 'replace', 'insert', 'delete']);
          const words = textOf(target).split(' ').filter(w => w && textOf(target).split(w).length === 2);
          let ops;
          if (kind === 'edit' && words.length) ops = [{ op: 'edit', id: target.id, find: pick(words), replace: token }];
          else if (kind === 'replace' || kind === 'edit') ops = [{ op: 'replace', id: target.id, markup: `<p id="${target.id}">${escapeXml(`${textOf(target)} ${token}`)}</p>` }];
          else if (kind === 'insert') ops = [{ op: 'insert', at: { after: target.id }, markup: `<p>${token} added</p>` }];
          else ops = [{ op: 'delete', id: target.id }];
          const batchId = `ab-${counter++}`;
          const focusAtApply = focused;
          const res = source.apply({ batchId, origin: agent.origin, base: agent.base, ops });
          action = `${agent.origin.id} ${ops[0].op} ${target.id} @${agent.base} -> ${errorsIn(res.problems)[0]?.code ?? (res.suggestionId ? 'suggestion' : 'direct')}`;
          const current = doc().blocks.find(b => b.id === target.id);
          const changedSince = JSON.stringify(before.blocks.find(b => b.id === target.id)) !== JSON.stringify(target);
          const errs = errorsIn(res.problems);
          if (changedSince && errs.length === 0) fail(step, `${action}: the block changed since the agent read it, yet the op went through`);
          if (errs.length === 0 && !res.suggestionId) {
            if (focusAtApply && focusAtApply === target.id) fail(step, `${action}: the block the person is editing was rewritten under them`);
            agent.batches.push(batchId);
            if (kind === 'insert') {
              const minted = idsIn(doc()).filter(id => !idsIn(before).includes(id));
              for (const id of minted) if (everSeen.has(id)) fail(step, `${action}: minted id ${id} was already used`);
            }
          }
          if (errs.length === 0 && focusAtApply === target.id && !res.suggestionId && current) fail(step, `${action}: an edit to the focused block must become a suggestion`);
          // The agent never takes out a human token it had not read.
          const after = tokensIn(doc());
          for (const t of beforeTokens) {
            if (humanTokens.has(t) && !after.has(t) && (tokenSeenAt.get(t) ?? 0) > agent.base) fail(step, `${action} removed the person's ${t}, written after the agent read`);
          }
        }
      }
      log.push(`${step}: ${action}`);

      // Every step: the draft validates, ids are well-formed and unique, new ids are new.
      const real = source.getState();
      if (real.doc !== latest.doc || real.revision !== latest.revision) fail(step, `${action}: the document changed without notifying subscribers`);
      const now = real.doc;
      const problems = errorsIn(checkDocument(now));
      if (problems.length) fail(step, `${action}: the draft does not validate: ${JSON.stringify(problems)}`);
      const ids = idsIn(now);
      if (new Set(ids).size !== ids.length) fail(step, `${action}: duplicate ids`);
      for (const id of ids) if (!ID_RE.test(id)) fail(step, `${action}: bad id ${id}`);
      if (real.revision === beforeRev && JSON.stringify(now) !== JSON.stringify(before)) fail(step, `${action}: the document changed without a new revision`);
      for (const id of ids) everSeen.add(id);
      const nowTokens = tokensIn(now);
      for (const t of nowTokens) if (!beforeTokens.has(t)) tokenSeenAt.set(t, real.revision);
    }
    return log.length;
  });
}

describe('property: concurrent human and agent edits', () => {
  it(`${SEEDS} seeded sequences of ${STEPS} steps keep every invariant`, () => {
    for (let seed = 1; seed <= SEEDS; seed++) runSequence(seed);
  });
});

// Each was first found by the sequences above; kept as plain cases so they stay covered whatever the seeds hit.
describe('property: found cases', () => {
  const AGENT = AGENTS[0];
  const three = () => createDocument({ blocks: [paragraph('zero', 'p0'), paragraph('one', 'p1'), paragraph('two', 'p2')] });
  const human = (source, ops) => source.apply({ batchId: `h${Math.random()}`, origin: HUMAN, base: source.getState().revision, ops });

  it('a merge counts as a change to the block it folds into', () => {
    const source = createJinshuSource({ key: 'merge-stale', initialDoc: three() });
    const read = source.getState().revision;
    assert.deepEqual(errorsIn(human(source, [{ op: 'merge', id: 'p2' }]).problems), []);
    const res = source.apply({ batchId: 'a1', origin: AGENT, base: read, ops: [{ op: 'replace', id: 'p1', markup: '<p id="p1">rewritten</p>' }] });
    assert.equal(res.problems[0]?.code, 'STALE_BLOCK');
    assert.equal(plainText(source.getState().doc.blocks[1].runs), 'onetwo');
  });

  it('undo then redo puts an insert back where it was, after a later insert at the same place', () => {
    const source = createJinshuSource({ key: 'redo-order', initialDoc: three() });
    human(source, [{ op: 'insert', at: { after: 'p0' }, blocks: [{ id: 'mine', type: 'paragraph', runs: [{ text: 'mine' }] }] }]);
    source.apply({ batchId: 'a1', origin: AGENT, base: source.getState().revision, ops: [{ op: 'insert', at: { after: 'p0' }, markup: '<p id="theirs">theirs</p>' }] });
    const before = source.getState().doc;
    assert.deepEqual(errorsIn(source.undo().problems), []);
    assert.deepEqual(errorsIn(source.redo().problems), []);
    assert.deepEqual(source.getState().doc, before);
  });

  it("undoing an agent's delete does not fold a later merge into the restored block", () => {
    const doc = createDocument({ blocks: [...three().blocks, paragraph('three', 'p3')] }); // a delete under the large-edit guard
    const source = createJinshuSource({ key: 'merge-replay', initialDoc: doc });
    source.apply({ batchId: 'del', origin: AGENT, base: source.getState().revision, ops: [{ op: 'delete', id: 'p1' }] });
    human(source, [{ op: 'merge', id: 'p2' }]);
    const before = source.getState().doc;
    assert.equal(plainText(before.blocks[0].runs), 'zerotwo');
    assert.equal(source.undo('del', AGENT).problems[0]?.code, 'REJECTED');
    assert.deepEqual(source.getState().doc, before);
  });
});
