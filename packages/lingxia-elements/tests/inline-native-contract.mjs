import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { compileInlineNativeRoot } from '../dist/inline-native/structure.js';
import { identifyCompiledRoot } from '../dist/inline-native/identity.js';
import { buildRootCommit } from '../dist/inline-native/commit.js';

const rootRef = { surfaceInstanceId: 's', pageInstanceId: 'p', documentInstanceId: 'd', rootKey: 'r', rootEpoch: 1 };
const button = (id, label = id) => ({ type: 'LxNativeButton', authorId: id, props: { label } });
const view = (id, children) => ({ type: 'LxNativeView', authorId: id, children });
const scenarios = {
  'sort-and-update': [[button('a'), button('b')], [button('b', 'New label'), button('a')]],
  'remove-wrapper': [[view('wrapper', [button('play')])], [button('play')]],
  'replace-wrapper': [[view('old', [button('play')])], [view('new', [button('play', 'Pause')])]],
  'reverse-ancestry': [[view('a', [view('b', [button('play')])])], [view('b', [view('a', [button('play')])])]],
  'change-kind': [[{ type: 'LxNativeText', authorId: 'status', children: 'Loading' }], [button('status', 'Retry')]],
  'remove-and-reinsert': [[button('a'), button('b')], [button('b')], [button('a'), button('b')]],
};
const fixtures = Object.entries(scenarios).map(([name, trees]) => {
  let previous = null;
  const commits = trees.map((children, index) => {
    const compiled = compileInlineNativeRoot({ type: 'LxNativeRoot', children });
    assert.equal(compiled.ok, true);
    const next = identifyCompiledRoot(compiled.root, rootRef, previous);
    const commit = buildRootCommit(next, previous, index + 1);
    previous = next;
    return commit;
  });
  const nodes = [];
  const walk = children => children.forEach(node => {
    nodes.push({ key: node.nodeRef.nodeKey, parent: node.parentRef?.nodeKey ?? null, order: node.order,
      kind: node.node.kind, props: node.node.text !== undefined ? { ...node.node.props, text: node.node.text } : node.node.props });
    walk(node.children);
  });
  walk(previous.children);
  return { name, commits, nodes };
});
const location = new URL('./fixtures/inline-native-commits.json', import.meta.url);
if (process.argv.includes('--update')) {
  await mkdir(new URL('./fixtures/', import.meta.url), { recursive: true });
  await writeFile(location, JSON.stringify(fixtures, null, 2) + '\n');
} else {
  assert.deepEqual(JSON.parse(JSON.stringify(fixtures)), JSON.parse(await readFile(location, 'utf8')), 'JS wire fixtures changed; update and run the Rust contract test');
}
