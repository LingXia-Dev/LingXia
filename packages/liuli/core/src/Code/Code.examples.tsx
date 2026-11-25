import { useLocal } from '@lingxia/view';
import { Code, CodeBlock } from './Code';

const install = `npm install @liulidesign/core
npx lingxia dev --platform ios`;

const response = `{
  "id": "note_8f2c",
  "title": "Trip to the coast",
  "updatedAt": "2026-11-14T09:42:00Z",
  "sharedWith": ["ana@example.com", "kai@example.com"]
}`;

/** A command to copy: the title says what it is, and the block's copy button hands the code to the app's clipboard call. */
export function Command() {
  // Stands in for the app's clipboard call.
  const clipboard = useLocal('clipboard', '');
  return (
    <CodeBlock
      code={install}
      language="bash"
      label="Install commands"
      title="Terminal"
      onCopy={code => clipboard.set(code)}
      data-testid="install"
    />
  );
}

/** Without a header: an example response whose long line scrolls sideways. */
export function Plain() {
  return <CodeBlock code={response} language="json" label="Example response" data-testid="response" />;
}

const excerpt = `export function total(items: readonly Item[]) {
  let sum = 0;
  for (const item of items) sum += item.price * item.count;
  return Math.round(sum * 100) / 100;
}`;

/** An excerpt with its numbers in the file, two lines marked. */
export function Numbered() {
  return <CodeBlock code={excerpt} language="ts" label="total in cart.ts" title="cart.ts" hasLineNumbers startLineNumber={41} highlightLines={[43, 44]} data-testid="numbered" />;
}

const change = `--- a/cart.ts
+++ b/cart.ts
@@ -42,4 +42,4 @@
   let sum = 0;
   for (const item of items)
-    sum += item.price;
+    sum += item.price * item.count;
   return Math.round(sum * 100) / 100;`;

/** A change as a unified diff: each sign in its own column, added and removed lines tinted. */
export function Diff() {
  return <CodeBlock code={change} language="ts" label="Change to cart.ts" title="cart.ts" isDiff data-testid="diff" />;
}

const config = `export default {
  name: 'notes',
  version: '1.4.0',
  pages: ['pages/home/index', 'pages/note/index', 'pages/settings/index'],
  window: { title: 'Notes', theme: 'system' },
  network: { timeout: 15000, retries: 2 },
  storage: { quota: '50MB', encrypt: true },
  share: { targets: ['link', 'image'] },
  sync: { interval: 300, onCellular: false },
  logs: { level: 'warn' },
};`;

/** A long file in a short box: past `maxHeight` the code scrolls up and down under the title's line. */
export function Capped() {
  const clipboard = useLocal('clipboard', '');
  return <CodeBlock code={config} language="ts" label="lxapp.config.ts" title="lxapp.config.ts" maxHeight={160} onCopy={code => clipboard.set(code)} data-testid="capped" />;
}

const prompt = 'Summarise the note in three short bullet points, keep every date and amount exactly as written, and end with one line listing anything that still needs an answer.';

/** Prose-like text, such as a prompt, wrapped at the block's width rather than scrolled. */
export function Wrapped() {
  return <CodeBlock code={prompt} label="Prompt" title="Prompt" isWrapped data-testid="wrapped" />;
}

/** In a sentence: a file name and a value to type exactly. */
export function InSentence() {
  return (
    <text className="text-body text-text">
      Set <Code text="minRuntime" data-testid="key" /> in <Code text="lxapp.json" /> to the oldest runtime you test on.
    </text>
  );
}

/** The home tile. */
export function Specimen() {
  return <CodeBlock code={'const a = 1;\nconst b = 2;'} label="Sample" className="w-36" />;
}
