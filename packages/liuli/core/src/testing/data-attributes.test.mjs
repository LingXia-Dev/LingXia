import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { renderPage } from './harness.mjs';

const playground = fileURLToPath(new URL('../../../gallery/shared/playground.generated.tsx', import.meta.url));
const core = fileURLToPath(new URL('..', import.meta.url));

// Overlays draw nothing while closed, so they are rendered open.
const opened = ['BottomSheet', 'CommandPalette', 'Dialog', 'Drawer', 'DropdownMenu', 'Lightbox', 'Popover', 'Tooltip'];
// Entries that still put no test id on screen, each with why. Keep it short.
const unseen = {
  // With a Button as its trigger, the Button keeps its own test id and these props reach no element: open in the audit.
  DropdownMenu: 'trigger is a pressable child',
  Popover: 'trigger is a pressable child',
  Tooltip: 'trigger is a pressable child',
};
// Components with no playground entry, given here directly where that is cheap.
const direct = {
  Link: `import { Link } from ${JSON.stringify(`${core}Link/Link`)};\nconst subject = (props: any) => <Link label="Terms" onClick={() => {}} {...props} />;`,
  SplitView: `import { SplitView } from ${JSON.stringify(`${core}SplitView/SplitView`)};\nconst subject = (props: any) => <SplitView sidebarLabel="Mailboxes" sidebar={<text>Inbox</text>} {...props}><text>Message</text></SplitView>;`,
};

/** What the page puts on screen: for each node whose test id is exactly `probe-<name>`, whether it carries the probe too. */
async function probe(page) {
  const screen = await renderPage(() => page);
  const found = new Map();
  for (const node of screen.nodes) {
    // Exactly the id given; ids a component derives for its parts (`probe-Calendar-next`) carry no copy.
    const name = /^probe-([A-Za-z]+)$/.exec(node.testId ?? '')?.[1];
    if (name) found.set(name, (found.get(name) ?? true) && JSON.parse(node.props.dataAttributes ?? '{}').probe === name);
  }
  return found;
}

const probes = name => `{ 'data-testid': 'probe-${name}', 'data-probe': '${name}' }`;

// JSX does not type-check hyphenated props, so a component that dropped `data-*` would drop it silently.
test('every component puts the data-* props it is given where it puts its test id', async () => {
  const found = await probe(`import { PlaygroundSubject, playgroundEntries } from ${JSON.stringify(playground)};
const opened = ${JSON.stringify(opened)};
export function render() {
  return <page><scroll>{playgroundEntries.map(entry => <view key={entry.name}><PlaygroundSubject name={entry.name} props={{ ...entry.defaults, ...(opened.includes(entry.name) ? { isOpen: true } : {}), 'data-testid': 'probe-' + entry.name, 'data-probe': entry.name }} set={() => {}} /></view>)}</scroll></page>;
}
`);
  for (const [name, source] of Object.entries(direct)) {
    for (const [key, value] of await probe(`${source}\nexport function render() { return <page>{subject(${probes(name)})}</page>; }\n`)) found.set(key, value);
  }
  const names = [...(await namesOf()), ...Object.keys(direct)];
  const missing = names.filter(name => !found.has(name) && !(name in unseen));
  const dropped = [...found].filter(([, carried]) => !carried).map(([name]) => name);
  assert.deepEqual({ missing, dropped }, { missing: [], dropped: [] });
});

/** The playground's entry names, read from its source so the test needs no TSX loader. */
async function namesOf() {
  const { readFileSync } = await import('node:fs');
  return [...readFileSync(playground, 'utf8').matchAll(/^ {4}"name": "([A-Za-z]+)"/gm)].map(match => match[1]);
}
