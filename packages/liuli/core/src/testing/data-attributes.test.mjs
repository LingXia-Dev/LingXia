import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { renderPage } from './harness.mjs';

const playground = fileURLToPath(new URL('../../../gallery/shared/playground.generated.tsx', import.meta.url));

// JSX does not type-check hyphenated props, so a component that dropped `data-*` would drop it silently.
test('every component puts the data-* props it is given where it puts its test id', async () => {
  const screen = await renderPage(() => `import { PlaygroundSubject, playgroundEntries } from ${JSON.stringify(playground)};
export function render() {
  return <page><scroll>{playgroundEntries.map(entry => <view key={entry.name}><PlaygroundSubject name={entry.name} props={{ ...entry.defaults, 'data-testid': 'probe-' + entry.name, 'data-probe': entry.name }} set={() => {}} /></view>)}</scroll></page>;
}
`);
  const dropped = [];
  let seen = 0;
  const walk = node => {
    // Exactly the id given; ids a component derives for its parts (`probe-Calendar-next`) carry no copy.
    if (/^probe-[A-Za-z]+$/.test(node.testId ?? '')) { seen++; if (JSON.parse(node.props.dataAttributes ?? '{}').probe === undefined) dropped.push(node.testId.slice(6)); }
    node.children?.forEach(walk);
  };
  walk(screen.root);
  assert.ok(seen > 40, `only ${seen} components rendered their test id`);
  assert.deepEqual(dropped, []);
});
