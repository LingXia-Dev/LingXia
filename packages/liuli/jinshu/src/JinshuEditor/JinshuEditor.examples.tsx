import { useLocal } from '@lingxia/view';
import { callout, codeBlock, createDocument, heading, list, listItem, paragraph, table } from '../schema/helpers';
import { applyOps } from '../schema/ops';
import { JinshuEditor } from './JinshuEditor';

const draft = createDocument({
  title: 'Limits',
  blocks: [
    heading(2, 'What a limit says', 'h1'),
    paragraph('A limit describes where a function heads.', 'p1'),
    list('bullet', [listItem('Definition', { id: 'i1' }), listItem('Exercises', { id: 'i2' })], { id: 'l1' }),
    callout('tip', [paragraph('Sketch the graph before you compute.', 'c1p')], { title: 'Tip', id: 'c1' }),
    table(['Week', 'Topic', 'Due'], [['3', 'Limits', 'Friday'], ['4', 'Continuity', 'Next Friday']], { id: 't1' }),
    codeBlock('ts', 'const f = (x: number) => x * x;\nf(2);', { id: 'k1', filename: 'square.ts' }),
  ],
});

const notes = createDocument({
  title: 'Meeting notes',
  blocks: [heading(2, 'Decisions', 'n1'), paragraph('Ship the reader first; the editor follows in May.', 'n2'), paragraph('Open: who owns the migration?', 'n3')],
});

/** The editor on a document the page holds: each edit comes back as ops the page applies. */
export function Draft() {
  const doc = useLocal('editor-example-doc', draft);
  return <JinshuEditor doc={doc.value} onOps={ops => doc.set(applyOps(doc.value, ops).doc)} hasLeaveGuard={false} data-testid="draft" />;
}

/** Each block's menu offers to hand it to the app's agent; the app opens its own conversation with it. */
export function AskAgent() {
  const doc = useLocal('editor-ask-doc', notes);
  const asked = useLocal<string>('editor-ask-ids', '');
  return (
    <view className="gap-3">
      <JinshuEditor
        doc={doc.value}
        onOps={ops => doc.set(applyOps(doc.value, ops).doc)}
        onAskAgent={ids => asked.set(ids.join(', '))}
        hasLeaveGuard={false}
        data-testid="ask-agent"
      />
      {asked.value !== '' && <text className="text-footnote text-muted">{`Handed to the assistant: ${asked.value}`}</text>}
    </view>
  );
}

/** Read-only: the document with no editing chrome. */
export function ReadOnly() {
  return <JinshuEditor doc={draft} isReadOnly data-testid="read-only" />;
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-40 gap-1.5 rounded-control border border-line bg-surface p-2.5">
      <text className="text-footnote font-semibold text-text">Limits</text>
      <view className="flex-row items-center gap-1.5">
        <view className="w-0.75 h-4 rounded-full bg-accent" />
        <text className="text-caption text-text">Where a function heads.</text>
      </view>
      <view className="h-1.5 w-24 rounded-full bg-text/8" />
    </view>
  );
}
