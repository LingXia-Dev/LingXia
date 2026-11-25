import { callout, createDocument, heading, paragraph } from '../schema/helpers';
import { JinshuDiff } from './JinshuDiff';

const saved = createDocument({
  blocks: [heading(2, 'Limits', 'h1'), paragraph('A limit says where a function goes.', 'p1'), paragraph('Old note.', 'p2'), paragraph('Unchanged.', 'p3')],
});
const draft = createDocument({
  blocks: [heading(2, 'Limits', 'h1'), paragraph('A limit says where a function heads.', 'p1'), paragraph('Unchanged.', 'p3'), paragraph('New exercise.', 'p4')],
});

const policy = createDocument({
  blocks: [
    heading(2, 'Refunds', 'r1'),
    paragraph('Refunds are issued within 30 days of purchase.', 'r2'),
    callout('info', [paragraph('Digital items are not refundable once downloaded.', 'r3p')], { title: 'Note', id: 'r3' }),
  ],
});
const revised = createDocument({
  blocks: [
    heading(2, 'Refunds', 'r1'),
    paragraph('Refunds are issued within 14 days of purchase, to the original payment method.', 'r2'),
    callout('warning', [paragraph('Digital items are not refundable once downloaded.', 'r3p')], { title: 'Note', id: 'r3' }),
  ],
});

/** A draft against the saved version: one block changed, one removed, one added. */
export function Revision() {
  return <JinshuDiff a={saved} b={draft} data-testid="diff" />;
}

/** Inline: every changed text shows its words struck and inserted in place; a change with no text, like a callout's tone, shows before and after. */
export function Inline() {
  return <JinshuDiff a={policy} b={revised} mode="inline" data-testid="inline" />;
}

/** Block: each changed block shown before and after, for edits too heavy to read word by word. */
export function Block() {
  return <JinshuDiff a={policy} b={revised} mode="block" data-testid="block" />;
}

/** Two equal documents. */
export function NoChanges() {
  return <JinshuDiff a={saved} b={saved} data-testid="same" />;
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="w-40 gap-1 rounded-control border border-text/12 bg-surface p-2.5">
      <text className="text-caption text-danger line-through bg-danger-soft">within 30 days</text>
      <text className="text-caption text-success bg-success-soft">within 14 days</text>
    </view>
  );
}
