import assert from 'node:assert/strict';
import { compileInlineNativeRoot } from '../dist/inline-native/structure.js';
import { identifyCompiledRoot } from '../dist/inline-native/identity.js';
import { buildRootCommit } from '../dist/inline-native/commit.js';

const rootRef = { surfaceInstanceId: 's', pageInstanceId: 'p', documentInstanceId: 'd', rootKey: 'r', rootEpoch: 1 };
const compile = children => {
  const result = compileInlineNativeRoot({ type: 'LxNativeRoot', children });
  assert.equal(result.ok, true);
  return result.root;
};
const buttons = ids => compile(ids.map(authorId => ({ type: 'LxNativeButton', authorId, props: { label: authorId } })));
const first = identifyCompiledRoot(buttons(['a', 'b']), rootRef);
const inserted = identifyCompiledRoot(buttons(['new', 'a', 'b']), rootRef, first);
assert.equal(new Set(inserted.children.map(n => n.nodeRef.nodeKey)).size, 3);
assert.equal(inserted.children[1].nodeRef.nodeKey, first.children[0].nodeRef.nodeKey);
assert.equal(inserted.children[2].nodeRef.nodeKey, first.children[1].nodeRef.nodeKey);
const insertion = buildRootCommit(inserted, first, 2);
assert.deepEqual(insertion.operations.filter(op => op.op === 'mount').map(op => op.node.authorId), ['new']);

const reordered = identifyCompiledRoot(buttons(['b', 'new', 'a']), rootRef, inserted);
const reorder = buildRootCommit(reordered, inserted, 3);
assert.ok(reorder.operations.every(op => op.op === 'reorder'));
const removed = identifyCompiledRoot(buttons(['b', 'a']), rootRef, reordered);
assert.equal(buildRootCommit(removed, reordered, 4).operations.filter(op => op.op === 'unmount').length, 1);
const reinserted = identifyCompiledRoot(buttons(['b', 'new', 'a']), rootRef, removed);
assert.notEqual(reinserted.children[1].nodeRef.nodeKey, inserted.children[0].nodeRef.nodeKey);

for (const nextId of ['status', 'retry', undefined]) {
  const old = identifyCompiledRoot(compile([{ type: 'LxNativeText', authorId: nextId === undefined ? undefined : 'status', children: 'Loading' }]), rootRef);
  const next = identifyCompiledRoot(compile([{ type: 'LxNativeButton', authorId: nextId, props: { label: 'Retry' } }]), rootRef, old);
  const operations = buildRootCommit(next, old, 2).operations;
  assert.deepEqual(operations.map(op => op.op), ['unmount', 'mount']);
  assert.equal(operations[1].node.kind, 'tappable');
}
