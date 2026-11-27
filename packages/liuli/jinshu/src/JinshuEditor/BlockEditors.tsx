import { cx, useLocal, useView } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Icon } from '@liulidesign/core/Icon';
import { Tooltip } from '@liulidesign/core/Tooltip';
import { IconButton } from '@liulidesign/core/IconButton';
import { newBlockId, paragraph, plainText, tableColumnCount } from '../schema/helpers';
import { formatAllows } from '../schema/index';
import type {
  AnyJinshuOp,
  AudioBlock,
  CalloutBlock,
  CalloutTone,
  CodeBlock,
  ColumnsBlock,
  DetailsBlock,
  FileBlock,
  ImageBlock,
  ImageSize,
  Inline,
  InlineAtom,
  JinshuBlock,
  JinshuFormat,
  ListBlock,
  ListStyle,
  MathBlock,
  ParagraphBlock,
  QuoteBlock,
  TableAlign,
  TableBlock,
  VideoBlock,
} from '../schema/types';
import { renderInlineRuns } from '../internal/InlineRuns';
import { CodeFrame, CodeLineNumbers, TableFrame, tableColumnFlex } from '../internal/Blocks';
import { MathFormulaView } from '../internal/MathView';
import { jinshuViewRecipe as r } from '../JinshuView/JinshuView.recipe';
import { jinshuText } from '../i18n/text';
import { blockEditorsRecipe as b } from './BlockEditors.recipe';
import { jinshuEditorRecipe } from './JinshuEditor.recipe';
import { SHORTHAND_MARKS, atomsIn, splitAt, inlinesToShorthand as toShorthand, shorthandToInlines as fromShorthand, type TextRange } from './inlineMarkdown';

const m = jinshuEditorRecipe.mathEditor;

const TICK_PATH = [
  { d: 'M3.5 8.5l3 3 6-7', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' },
] as const;

export interface BlockEditorCallbacks {
  /** The editor's test id, under which the block editors' own test ids hang. */
  readonly testScope?: string;
  /** The document's file format: fields it cannot hold are not offered, and shorthand follows its dialect. */
  readonly format?: JinshuFormat;
  readonly dispatchOps: (ops: readonly AnyJinshuOp[]) => void;
  readonly onFocusBlock: (blockId: string) => void;
  readonly onBlurBlock: () => void;
  /** Puts the caret in the empty paragraph `blockId`, once a batch has made it. */
  readonly onEditParagraph?: (blockId: string) => void;
  /** A block a line-start rule just made from the paragraph being typed: its editor opens holding `text`. */
  readonly opening?: { readonly id: string; readonly text: string };
  /** An editor holding a field open hands over how to save and close it, and `null` once closed: a toolbar insert closes it first. */
  readonly holdOpen?: (blockId: string, close: (() => void) | null, save?: () => void) => void;
  /** The person typed in a field whose text is not in the document yet. */
  readonly onTyping?: () => void;
}

/** The text a just-made block opens with, when `block` is it. */
const openingText = (cb: BlockEditorCallbacks, id: string): string | undefined => (cb.opening?.id === id ? cb.opening.text : undefined);

const dialectOf = (format: JinshuFormat | undefined) => ({ dialect: format === 'markdown' ? 'markdown' : 'jinshu' } as const);

/** Shorthand in the dialect of the document's format. */
function inlinesToShorthand(runs: readonly Inline[], format: JinshuFormat | undefined): string {
  return toShorthand(runs, dialectOf(format));
}

function shorthandToInlines(
  text: string,
  format: JinshuFormat | undefined,
  options: { readonly inHeading?: boolean; readonly existingAtoms?: readonly InlineAtom[] },
): Inline[] {
  return fromShorthand(text, { ...options, ...dialectOf(format) });
}

/** Whether the document's format holds `field` on a block of `type`. */
const holds = (cb: BlockEditorCallbacks, type: string, field: string) => formatAllows(cb.format ?? 'json', { block: type, field });

// ---------------------------------------------------------------------------
// Container bodies: every paragraph child is editable, blank line = new paragraph
// ---------------------------------------------------------------------------

function isTextChild(b: JinshuBlock): b is ParagraphBlock {
  return b.type === 'paragraph';
}

/** Paragraph children joined by blank lines; non-paragraph children are kept as-is and counted. */
function containerBodyText(blocks: readonly JinshuBlock[], format: JinshuFormat | undefined): { text: string; kept: number } {
  const paras = blocks.filter(isTextChild);
  return { text: paras.map(p => inlinesToShorthand(p.runs, format)).join('\n\n'), kept: blocks.length - paras.length };
}

/** Rebuilds container children: paragraphs from blank-line-separated text (ids reused in order), other children kept in place. */
function rebuildContainerBody(blocks: readonly JinshuBlock[], text: string, format: JinshuFormat | undefined): JinshuBlock[] {
  const parts = text
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/)
    .map(s => s.replace(/^\n+|\n+$/g, ''))
    .filter(s => s.length > 0);
  const oldParas = blocks.filter(isTextChild);
  const existingAtoms = oldParas.flatMap(p => atomsIn(p.runs));
  const newParas: JinshuBlock[] = (parts.length > 0 ? parts : ['']).map((part, idx) =>
    paragraph(shorthandToInlines(part, format, { existingAtoms }), oldParas[idx]?.id ?? newBlockId('p')),
  );
  // Paragraphs take the slots of the old paragraphs; extra ones follow the last slot; others keep their order.
  const out: JinshuBlock[] = [];
  let used = 0;
  let lastParaSlot = -1;
  blocks.forEach(b => {
    if (isTextChild(b)) {
      if (used < newParas.length) out.push(newParas[used++]);
      lastParaSlot = out.length;
    } else {
      out.push(b);
    }
  });
  if (used < newParas.length) {
    const rest = newParas.slice(used);
    if (lastParaSlot < 0) out.unshift(...rest);
    else out.splice(lastParaSlot, 0, ...rest);
  }
  return out;
}

function KeptChildrenNote({ count }: { readonly count: number }) {
  if (count === 0) return null;
  return <text className={b.keptNote}>{jinshuText('@jinshu.editor.block.keptChildren', { count })}</text>;
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

/** Direct-manipulation editor for bullet / ordered / task lists. */
export function ListBlockEditor({ block, cb }: { readonly block: ListBlock; readonly cb: BlockEditorCallbacks }) {
  const opening = openingText(cb, block.id);
  const activeItemId = useLocal<string | null>(`ed-li-${block.id}`, opening !== undefined ? (block.items[0]?.id ?? null) : null);
  const itemDraft = useLocal<string>(`ed-li-txt-${block.id}`, opening ?? '');
  const start = block.start ?? 1;

  // Left with one empty item, the list was only started: it goes, as an empty quote or code block does.
  const isAbandoned = () => block.items.length === 1 && itemDraft.value.trim() === '' && !block.items[0].blocks?.length;
  const drop = () => cb.dispatchOps([{ op: 'delete', id: block.id, noteWord: { key: '@jinshu.note.delete' } }]);
  const commitItem = (itemId: string, text: string) => {
    const item = block.items.find(it => it.id === itemId);
    if (!item || inlinesToShorthand(item.runs, cb.format) === text) return;
    cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, items: block.items.map(it => (it.id === itemId ? { ...it, runs: shorthandToInlines(text, cb.format, { existingAtoms: atomsIn(it.runs) }) } : it)) }] }]);
  };

  cb.holdOpen?.(
    block.id,
    activeItemId.value === null
      ? null
      : () => {
          if (isAbandoned()) drop();
          else if (activeItemId.value !== null) commitItem(activeItemId.value, itemDraft.value);
          activeItemId.set(null);
          cb.onBlurBlock();
        },
    () => {
      if (activeItemId.value !== null) commitItem(activeItemId.value, itemDraft.value);
    },
  );

  const focusItem = (idx: number) => {
    const target = block.items[idx];
    if (!target) return;
    activeItemId.set(target.id);
    itemDraft.set(inlinesToShorthand(target.runs, cb.format));
  };

  return (
    <view className={b.list.root}>
      {activeItemId.value !== null && (
        <view className={b.list.styleBar}>
          {(['bullet', 'ordered', 'task'] as ListStyle[]).map(st => (
            <Button
              key={st}
              label={jinshuText('@jinshu.editor.block.listStyle', { style: st })}
              size="sm"
              variant={block.style === st ? 'tonal' : 'ghost'}
              onClick={() => cb.dispatchOps([{ op: 'update', id: block.id, attrs: { style: st }, noteWord: { key: '@jinshu.note.listStyle' } }])}
            />
          ))}
          <text className={b.list.styleHint}>{jinshuText('@jinshu.editor.block.listHint')}</text>
        </view>
      )}

      <view className={r.list.base}>
        {block.items.map((item, idx) => {
          const isEditing = activeItemId.value === item.id;
          return (
            <view key={item.id} className={b.list.item}>
              {block.style === 'task' ? (
                <button
                  role="menuitemcheckbox"
                  aria-checked={item.checked === true}
                  aria-label={plainText(item.runs) || jinshuText('@jinshu.editor.block.taskItem')}
                  onClick={() => cb.dispatchOps([{ op: 'update', id: item.id, attrs: { checked: !(item.checked === true) }, noteWord: { key: '@jinshu.note.taskToggle' } }])}
                  className={cx(r.list.checkbox, item.checked ? r.list.checkboxDone : r.list.checkboxOpen)}
                >
                  {item.checked && <vector aria-hidden paths={TICK_PATH} viewBox={[0, 0, 16, 16]} className={b.list.tick} />}
                </button>
              ) : (
                <text aria-hidden className={r.list.marker}>
                  {block.style === 'ordered' ? `${start + idx}.` : '•'}
                </text>
              )}

              <view className={b.list.body}>
                {isEditing ? (
                  // One row that grows with its text, so the open item keeps the line height it is read at.
                  <textarea
                    aria-label={jinshuText('@jinshu.editor.block.editItem', { index: idx + 1 })}
                    value={itemDraft.value}
                    autoFocus
                    rows={1}
                    maxRows={8}
                    placeholder={jinshuText('@jinshu.editor.block.itemPlaceholder')}
                    onInput={v => {
                      // `- [ ] ` typed key by key is a bullet at `- `; the box that follows makes it a task list.
                      const box = block.style === 'bullet' && block.items.length === 1 ? /^\[( |x|X)\] (.*)$/.exec(v) : null;
                      if (box) {
                        const checked = box[1] !== ' ';
                        cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, style: 'task', items: [{ ...item, checked, runs: shorthandToInlines(box[2], cb.format, { existingAtoms: atomsIn(item.runs) }) }] }], noteWord: { key: '@jinshu.note.listStyle' } }]);
                        itemDraft.set(box[2]);
                        return;
                      }
                      itemDraft.set(v);
                      cb.onTyping?.();
                    }}
                    onBlur={() => {
                      if (activeItemId.value === item.id && isAbandoned()) drop();
                      else commitItem(item.id, itemDraft.value);
                      if (activeItemId.value === item.id) activeItemId.set(null);
                      cb.onBlurBlock();
                    }}
                    onKeyDown={{
                      Enter: () => {
                        // An empty item ends the list there: a paragraph takes its place, and items after it form a new list.
                        if (itemDraft.value.trim() === '' && !item.blocks?.length) {
                          const before = block.items.slice(0, idx);
                          const after = block.items.slice(idx + 1);
                          // The list's id stays with whatever comes first, so a reference to it still lands here.
                          const exitId = before.length > 0 ? newBlockId('p') : block.id;
                          const { start: _start, ...rest } = block;
                          const blocks: JinshuBlock[] = [
                            ...(before.length > 0 ? [{ ...block, items: before }] : []),
                            paragraph('', exitId),
                            ...(after.length > 0 ? [{ ...rest, id: newBlockId('l'), items: after }] : []),
                          ];
                          cb.dispatchOps([{ op: 'replace', id: block.id, blocks, noteWord: { key: '@jinshu.note.listExit' } }]);
                          activeItemId.set(null);
                          cb.onEditParagraph?.(exitId);
                          return true;
                        }
                        const newId = newBlockId('li');
                        const nextItems = [...block.items];
                        nextItems[idx] = { ...item, runs: shorthandToInlines(itemDraft.value, cb.format, { existingAtoms: atomsIn(item.runs) }) };
                        nextItems.splice(idx + 1, 0, block.style === 'task' ? { id: newId, runs: [], checked: false } : { id: newId, runs: [] });
                        cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, items: nextItems }] }]);
                        activeItemId.set(newId);
                        itemDraft.set('');
                        return true;
                      },
                      ArrowUp: () => {
                        if (idx === 0) return false;
                        commitItem(item.id, itemDraft.value);
                        focusItem(idx - 1);
                        return true;
                      },
                      ArrowDown: () => {
                        if (idx >= block.items.length - 1) return false;
                        commitItem(item.id, itemDraft.value);
                        focusItem(idx + 1);
                        return true;
                      },
                      Escape: () => {
                        commitItem(item.id, itemDraft.value);
                        activeItemId.set(null);
                        cb.onBlurBlock();
                        return true;
                      },
                    }}
                    className={cx(b.list.input, r.paragraph.base)}
                  />
                ) : (
                  <button
                    aria-label={jinshuText('@jinshu.editor.block.editItem', { index: idx + 1 })}
                    onClick={() => {
                      cb.onFocusBlock(block.id);
                      focusItem(idx);
                    }}
                    className={b.list.itemButton}
                  >
                    {/* Inline marks (bold, code, math) stay visible outside editing. */}
                    <text className={cx(r.paragraph.base, item.checked && b.list.checked, item.runs.length === 0 && b.list.empty)}>
                      {item.runs.length > 0 ? renderInlineRuns(item.runs, { blockId: item.id }) : jinshuText('@jinshu.editor.block.emptyItem')}
                    </text>
                  </button>
                )}
              </view>

              {isEditing && block.items.length > 1 && (
                <view className={b.list.remove}>
                  <Button
                    label={jinshuText('@jinshu.editor.block.removeItem')}
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      cb.dispatchOps([{ op: 'delete', id: item.id, noteWord: { key: '@jinshu.note.listItemDelete' } }]);
                      activeItemId.set(null);
                    }}
                  />
                </view>
              )}
            </view>
          );
        })}
      </view>
    </view>
  );
}

// ---------------------------------------------------------------------------
// Callout / Quote / Details
// ---------------------------------------------------------------------------

function ContainerPreview({ blocks, muted }: { readonly blocks: readonly JinshuBlock[]; readonly muted?: boolean }) {
  return (
    <view className={b.container.preview}>
      {blocks.map(child =>
        child.type === 'paragraph' || child.type === 'heading' ? (
          <text key={child.id} className={cx(r.paragraph.base, muted && b.container.muted, child.type === 'heading' && b.container.heading)}>
            {child.runs.length > 0 ? renderInlineRuns(child.runs, { blockId: child.id }) : '\u200B'}
          </text>
        ) : (
          <text key={child.id} className={b.container.other}>{`[${child.type}]`}</text>
        ),
      )}
    </view>
  );
}

/** Direct-manipulation editor for Callout blocks (title, every body paragraph, tone). */
export function CalloutBlockEditor({ block, cb }: { readonly block: CalloutBlock; readonly cb: BlockEditorCallbacks }) {
  const opening = openingText(cb, block.id);
  const editing = useLocal(`ed-co-${block.id}`, opening !== undefined);
  const titleDraft = useLocal(`ed-co-t-${block.id}`, '');
  const bodyDraft = useLocal(`ed-co-b-${block.id}`, opening ?? '');
  const body = containerBodyText(block.blocks, cb.format);

  const openEdit = () => {
    cb.onFocusBlock(block.id);
    titleDraft.set(block.title ? inlinesToShorthand(block.title, cb.format) : '');
    bodyDraft.set(body.text);
    editing.set(true);
  };

  const save = () => {
    const nextTitle = titleDraft.value.trim() ? shorthandToInlines(titleDraft.value, cb.format, { existingAtoms: atomsIn(block.title) }) : undefined;
    const sameTitle = (block.title ? inlinesToShorthand(block.title, cb.format) : '') === titleDraft.value.trim();
    if (!sameTitle || bodyDraft.value !== body.text) {
      const next: Record<string, unknown> = { ...block, blocks: rebuildContainerBody(block.blocks, bodyDraft.value, cb.format) };
      if (nextTitle) next.title = nextTitle;
      else delete next.title;
      cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [next as unknown as CalloutBlock], noteWord: { key: '@jinshu.note.editCallout' } }]);
    }
  };
  const commit = () => {
    // Closed with nothing in it, the block was only started: it goes.
    if (!titleDraft.value.trim() && !bodyDraft.value.trim()) cb.dispatchOps([{ op: 'delete', id: block.id, noteWord: { key: '@jinshu.note.delete' } }]);
    else save();
    editing.set(false);
    cb.onBlurBlock();
  };
  cb.holdOpen?.(block.id, editing.value ? commit : null, save);

  if (!editing.value) {
    return (
      <view className={cx(r.callout.base, r.callout.tone[block.tone], b.callout.preview)}>
        {/* As in the reader: without a title the tone names itself. */}
        <text className={r.callout.title[block.tone]}>
          {block.title && block.title.length > 0 ? renderInlineRuns(block.title, { blockId: block.id }) : jinshuText('@jinshu.calloutTone', { tone: block.tone })}
        </text>
        <ContainerPreview blocks={block.blocks} />
        <button aria-label={jinshuText('@jinshu.editor.block.editCallout', { id: block.id })} onClick={openEdit} className={b.callout.previewButton} />
      </view>
    );
  }

  return (
    <view className={cx(r.callout.base, r.callout.tone[block.tone], b.callout.editing)}>
      <view className={b.callout.header}>
        {/* An empty title shows the tone's name, which is what the reader shows in its place. */}
        {holds(cb, 'callout', 'title') ? (
          <input
            aria-label={jinshuText('@jinshu.editor.block.calloutTitle')}
            value={titleDraft.value}
            placeholder={jinshuText('@jinshu.calloutTone', { tone: block.tone })}
            onInput={v => { titleDraft.set(v); cb.onTyping?.(); }}
            className={cx(b.callout.titleInput, r.callout.title[block.tone])}
          />
        ) : (
          <text className={cx(b.callout.titleInput, r.callout.title[block.tone])}>{jinshuText('@jinshu.calloutTone', { tone: block.tone })}</text>
        )}
        <view className={b.callout.tones}>
          {(['info', 'tip', 'warning', 'danger'] as CalloutTone[]).map(tone => (
            <Button
              key={tone}
              label={jinshuText('@jinshu.calloutTone', { tone })}
              size="sm"
              variant={block.tone === tone ? 'tonal' : 'ghost'}
              onClick={() => cb.dispatchOps([{ op: 'update', id: block.id, attrs: { tone } }])}
            />
          ))}
        </view>
      </view>
      <textarea
        aria-label={jinshuText('@jinshu.editor.block.calloutBody')}
        value={bodyDraft.value}
        autoFocus
        rows={2}
        maxRows={18}
        placeholder={jinshuText('@jinshu.editor.block.calloutBodyPlaceholder')}
        onInput={v => { bodyDraft.set(v); cb.onTyping?.(); }}
        onKeyDown={{ Escape: () => { commit(); return true; } }}
        className={cx(b.callout.body, r.paragraph.base)}
      />
      <view className={b.callout.footer}>
        <KeptChildrenNote count={body.kept} />
        <Button label={jinshuText('@jinshu.editor.block.done')} size="sm" variant="tonal" onClick={commit} />
      </view>
    </view>
  );
}

/** Direct-manipulation editor for Quote blocks (every paragraph). */
export function QuoteBlockEditor({ block, cb }: { readonly block: QuoteBlock; readonly cb: BlockEditorCallbacks }) {
  const opening = openingText(cb, block.id);
  const editing = useLocal(`ed-q-${block.id}`, opening !== undefined);
  const draft = useLocal(`ed-q-t-${block.id}`, opening ?? '');
  const body = containerBodyText(block.blocks, cb.format);

  const save = () => {
    if (draft.value !== body.text) {
      cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, blocks: rebuildContainerBody(block.blocks, draft.value, cb.format) }], noteWord: { key: '@jinshu.note.editQuote' } }]);
    }
  };
  const commit = () => {
    // Closed with nothing in it, the block was only started: it goes.
    if (!draft.value.trim()) cb.dispatchOps([{ op: 'delete', id: block.id, noteWord: { key: '@jinshu.note.delete' } }]);
    else save();
    editing.set(false);
    cb.onBlurBlock();
  };
  cb.holdOpen?.(block.id, editing.value ? commit : null, save);

  if (!editing.value) {
    return (
      <view className={cx(r.quote.base, b.quote.preview)}>
        <ContainerPreview blocks={block.blocks} muted />
        <button
          aria-label={jinshuText('@jinshu.editor.block.editQuote', { id: block.id })}
          onClick={() => {
            cb.onFocusBlock(block.id);
            draft.set(body.text);
            editing.set(true);
          }}
          className={b.quote.previewButton}
        />
      </view>
    );
  }

  return (
    <view className={cx(r.quote.base, b.quote.editing)}>
      <textarea
        aria-label={jinshuText('@jinshu.editor.block.quoteBody')}
        value={draft.value}
        autoFocus
        rows={1}
        maxRows={18}
        placeholder={jinshuText('@jinshu.editor.block.quoteBodyPlaceholder')}
        onInput={v => { draft.set(v); cb.onTyping?.(); }}
        onBlur={commit}
        onKeyDown={{ Escape: () => { commit(); return true; } }}
        className={cx(b.quote.body, r.paragraph.base, b.quote.bodyText)}
      />
      <KeptChildrenNote count={body.kept} />
    </view>
  );
}

/** Direct-manipulation editor for Details blocks (summary + every body paragraph). */
export function DetailsBlockEditor({ block, cb }: { readonly block: DetailsBlock; readonly cb: BlockEditorCallbacks }) {
  const open = useLocal(`ed-det-open-${block.id}`, block.defaultOpen === true);
  const summary = inlinesToShorthand(block.summary, cb.format);
  const body = containerBodyText(block.blocks, cb.format);
  const summaryDraft = useFieldDraft(`ed-det-s-${block.id}`, summary);
  const bodyDraft = useFieldDraft(`ed-det-b-${block.id}`, body.text);

  const commit = () => {
    if (summaryDraft.value !== summary || bodyDraft.value !== body.text) {
      cb.dispatchOps([
        {
          op: 'replace',
          id: block.id,
          blocks: [{ ...block, summary: shorthandToInlines(summaryDraft.value, cb.format, { existingAtoms: atomsIn(block.summary) }), blocks: rebuildContainerBody(block.blocks, bodyDraft.value, cb.format) }],
          noteWord: { key: '@jinshu.note.editDetails' },
        },
      ]);
    }
    summaryDraft.settle();
    bodyDraft.settle();
  };
  // Its fields are always open, but only typed text is the person's: untouched, they follow the document.
  cb.holdOpen?.(block.id, summaryDraft.isEdited || bodyDraft.isEdited ? commit : null, commit);

  // The reader's geometry: the summary row with its chevron at the end; whether it starts open is a block-menu choice.
  return (
    <view>
      <view className={b.details.header}>
        <input
          aria-label={jinshuText('@jinshu.editor.block.detailsSummary')}
          value={summaryDraft.value}
          placeholder={jinshuText('@jinshu.editor.block.detailsSummaryPlaceholder')}
          onInput={v => { summaryDraft.set(v); cb.onTyping?.(); }}
          onFocus={() => cb.onFocusBlock(block.id)}
          onBlur={() => { commit(); cb.onBlurBlock(); }}
          className={b.details.summary}
        />
        <button
          aria-label={jinshuText('@jinshu.editor.block.detailsToggle')}
          aria-expanded={open.value}
          onClick={() => open.set(!open.value)}
          className={b.details.toggle}
        >
          <Icon name="chevron-down" className={cx(b.details.chevron.base, open.value && b.details.chevron.open)} />
        </button>
      </view>
      {open.value && (
        <view className={b.details.body}>
          <textarea
            aria-label={jinshuText('@jinshu.editor.block.detailsBody')}
            value={bodyDraft.value}
            rows={2}
            maxRows={18}
            placeholder={jinshuText('@jinshu.editor.block.detailsBodyPlaceholder')}
            onInput={v => { bodyDraft.set(v); cb.onTyping?.(); }}
            onFocus={() => cb.onFocusBlock(block.id)}
            onBlur={() => { commit(); cb.onBlurBlock(); }}
            className={cx(b.details.bodyInput, r.paragraph.base)}
          />
          <KeptChildrenNote count={body.kept} />
        </view>
      )}
    </view>
  );
}

// ---------------------------------------------------------------------------
// Code
// ---------------------------------------------------------------------------

/** Direct-manipulation editor for Code blocks (language, filename, multiline code). */
export function CodeBlockEditor({ block, cb }: { readonly block: CodeBlock; readonly cb: BlockEditorCallbacks }) {
  const editing = useLocal(`ed-code-${block.id}`, openingText(cb, block.id) !== undefined);
  const langDraft = useLocal(`ed-code-l-${block.id}`, block.language);
  const fileDraft = useLocal(`ed-code-f-${block.id}`, block.filename ?? '');
  const codeDraft = useLocal(`ed-code-c-${block.id}`, block.code);

  const save = () => {
    if (langDraft.value.trim() === block.language && codeDraft.value === block.code && fileDraft.value.trim() === (block.filename ?? '')) return;
    cb.dispatchOps([
      {
        op: 'update',
        id: block.id,
        attrs: {
          language: langDraft.value.trim() || 'text',
          ...(holds(cb, 'code', 'filename') ? { filename: fileDraft.value.trim() || null } : {}),
          code: codeDraft.value,
        },
        noteWord: { key: '@jinshu.note.editCode' },
      },
    ]);
  };
  const commit = () => {
    // Closed with nothing in it, the block was only started: it goes.
    if (!codeDraft.value.trim()) cb.dispatchOps([{ op: 'delete', id: block.id, noteWord: { key: '@jinshu.note.delete' } }]);
    else save();
    editing.set(false);
    cb.onBlurBlock();
  };
  cb.holdOpen?.(block.id, editing.value ? commit : null, save);

  if (!editing.value) {
    return (
      <CodeFrame
        block={block}
        testId={cb.testScope && `${cb.testScope}-block-${block.id}`}
        action={
          <Button
            label={jinshuText('@jinshu.editor.block.codeEdit')}
            startIcon="edit"
            variant="ghost"
            size="sm"
            onClick={() => {
              cb.onFocusBlock(block.id);
              langDraft.set(block.language);
              fileDraft.set(block.filename ?? '');
              codeDraft.set(block.code);
              editing.set(true);
            }}
          />
        }
      />
    );
  }

  // The same frame and gutter as the reader; the header's title becomes the file and language fields.
  return (
    <view className={cx(r.code.root, b.code.editing)}>
      <view className={r.code.header}>
        <view className={b.code.fields}>
          {holds(cb, 'code', 'filename') && (
            <>
              <input
                aria-label={jinshuText('@jinshu.editor.block.codeFilename')}
                value={fileDraft.value}
                placeholder={jinshuText('@jinshu.editor.block.codeFilenamePlaceholder')}
                onInput={v => { fileDraft.set(v); cb.onTyping?.(); }}
                className={b.code.filename}
              />
              <text aria-hidden className={r.code.lang}>·</text>
            </>
          )}
          <input
            aria-label={jinshuText('@jinshu.editor.block.codeLanguage')}
            value={langDraft.value}
            placeholder={jinshuText('@jinshu.editor.block.codeLanguagePlaceholder')}
            onInput={v => { langDraft.set(v); cb.onTyping?.(); }}
            className={b.code.language}
          />
        </view>
        <Button label={jinshuText('@jinshu.editor.block.done')} startIcon="check" size="sm" variant="ghost" onClick={commit} />
      </view>
      <view className={b.code.body}>
        <view className={b.code.lines}>
          <CodeLineNumbers count={codeDraft.value.split('\n').length} testId={cb.testScope && `${cb.testScope}-block-${block.id}-numbers`} />
          <textarea
            aria-label={jinshuText('@jinshu.editor.block.codeBody')}
            value={codeDraft.value}
            autoFocus
            rows={3}
            maxRows={24}
            onInput={v => { codeDraft.set(v); cb.onTyping?.(); }}
            onKeyDown={{ Escape: () => { commit(); return true; } }}
            className={b.code.input}
          />
        </view>
      </view>
    </view>
  );
}

// ---------------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------------

type TableSel = { readonly kind: 'row' | 'col'; readonly index: number } | null;
type CellPos = { readonly row: number; readonly col: number } | null;

const ALIGN_ICON = {
  start: 'align-start',
  center: 'align-center',
  end: 'align-end',
} as const;

/**
 * Table editor on the reader's frame and grid:
 * - A handle over each column and beside each row (on hover, or always on touch) selects that line.
 * - A selected line, or an active cell, gets a compact toolbar floating above the table; hovering Delete tints the doomed cells.
 * - Multi-line cells edit in place (`rows={1} maxRows={6}`) and keep inline formatting; ↑ ↓ move rows, Enter goes down.
 */
export function TableBlockEditor({ block, cb }: { readonly block: TableBlock; readonly cb: BlockEditorCallbacks }) {
  const { env } = useView();
  const activePos = useLocal<CellPos>(`ed-tbl-pos-${block.id}`, null);
  const sel = useLocal<TableSel>(`ed-tbl-sel-${block.id}`, null);
  const hovered = useLocal(`ed-tbl-hover-${block.id}`, false);
  const dangerHover = useLocal(`ed-tbl-danger-${block.id}`, false);
  const cellDraft = useLocal<string>(`ed-tbl-v-${block.id}`, '');

  const pos = activePos.value;
  const selection = sel.value;
  const isDanger = dangerHover.value && selection !== null;
  // The editor indexes body rows from 0 and the header row as -1; ops address rows by id.
  const headerRow = block.headerRow ? block.rows[0] : undefined;
  const bodyRows = headerRow ? block.rows.slice(1) : block.rows;
  const nCols = tableColumnCount(block);
  const nRows = bodyRows.length;
  // Worded here, in render: the column is inserted from a handler, where `jinshuText` cannot run.
  const newColumnHeader = jinshuText('@jinshu.editor.seed.column', { column: nCols + 1 });

  const rowAt = (row: number) => (row === -1 ? headerRow : bodyRows[row]);
  const touch = env.pointer === 'coarse';
  const chrome = touch || hovered.value || pos !== null || selection !== null;

  const cellRuns = (row: number, col: number) => rowAt(row)?.cells[col] ?? [];
  const cellText = (row: number, col: number) => inlinesToShorthand(cellRuns(row, col), cb.format);

  const focusCell = (row: number, col: number) => {
    cb.onFocusBlock(block.id);
    sel.set(null);
    dangerHover.set(false);
    activePos.set({ row, col });
    cellDraft.set(cellText(row, col));
  };

  const commitCell = (row: number, col: number, text: string) => {
    if (text === cellText(row, col)) return;
    const target = rowAt(row);
    if (!target) return;
    const runs = shorthandToInlines(text, cb.format, { existingAtoms: atomsIn(cellRuns(row, col)) });
    const rows = block.rows.map(r => (r.id === target.id ? { ...r, cells: r.cells.map((c, i) => (i === col ? runs : c)) } : r));
    cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, rows }], noteWord: { key: row === -1 ? '@jinshu.note.tableHeader' : '@jinshu.note.tableCell' } }]);
  };

  cb.holdOpen?.(
    block.id,
    pos === null
      ? null
      : () => {
          if (activePos.value) commitCell(activePos.value.row, activePos.value.col, cellDraft.value);
          activePos.set(null);
        },
    () => {
      if (activePos.value) commitCell(activePos.value.row, activePos.value.col, cellDraft.value);
    },
  );

  const selectLine = (kind: 'row' | 'col', index: number) => {
    if (pos) commitCell(pos.row, pos.col, cellDraft.value);
    activePos.set(null);
    dangerHover.set(false);
    cb.onFocusBlock(block.id);
    sel.set(selection && selection.kind === kind && selection.index === index ? null : { kind, index });
  };

  const setColAlign = (col: number, alignVal: TableAlign) => {
    const nextAlign: TableAlign[] = Array.from({ length: nCols }, (_, i) => block.align?.[i] ?? 'start');
    nextAlign[col] = alignVal;
    cb.dispatchOps([{ op: 'update', id: block.id, attrs: { align: nextAlign }, noteWord: { key: '@jinshu.note.columnAlign' } }]);
  };

  const insertCol = (index: number) => {
    cb.dispatchOps([{ op: 'tableCol', id: block.id, action: 'insert', index, header: [{ text: newColumnHeader }], noteWord: { key: '@jinshu.note.columnInsert' } }]);
  };
  const insertRow = (index: number) => {
    const anchor = bodyRows[index];
    cb.dispatchOps([{ op: 'tableRow', id: block.id, action: 'insert', ...(anchor ? { before: anchor.id } : {}), noteWord: { key: '@jinshu.note.rowInsert' } }]);
  };

  const lineToolbar =
    selection === null ? (
      pos === null ? null : (
        <scroll className={b.table.toolbar}>
          <view className={b.table.toolbarRow}>
            <Button label={jinshuText('@jinshu.editor.block.addRowButton')} startIcon="add" size="sm" variant="ghost" onClick={() => insertRow(nRows)} />
            <Button label={jinshuText('@jinshu.editor.block.addColumnButton')} startIcon="add" size="sm" variant="ghost" onClick={() => insertCol(nCols)} />
          </view>
        </scroll>
      )
    ) : selection.kind === 'col' ? (
      <scroll className={b.table.toolbar}>
        <view className={b.table.toolbarRow}>
          <text className={b.table.toolbarLabel}>{jinshuText('@jinshu.editor.block.column', { index: selection.index + 1 })}</text>
          {(['start', 'center', 'end'] as TableAlign[]).map(al => (
            <Tooltip key={al} content={jinshuText('@jinshu.editor.block.align', { align: al })} placement="bottom"><IconButton
              label={jinshuText('@jinshu.editor.block.align', { align: al })}
              icon={ALIGN_ICON[al]}
              size="sm"
              variant={(block.align?.[selection.index] ?? 'start') === al ? 'tonal' : 'ghost'}
              onClick={() => setColAlign(selection.index, al)}
            /></Tooltip>
          ))}
          <view aria-hidden className={b.table.toolbarSeparator} />
          <Button label={jinshuText('@jinshu.editor.block.insertLeft')} size="sm" variant="ghost" onClick={() => { insertCol(selection.index); sel.set({ kind: 'col', index: selection.index + 1 }); }} />
          <Button label={jinshuText('@jinshu.editor.block.insertRight')} size="sm" variant="ghost" onClick={() => insertCol(selection.index + 1)} />
          {selection.index > 0 && (
            <Button
              label={jinshuText('@jinshu.editor.block.moveLeft')}
              size="sm"
              variant="ghost"
              onClick={() => {
                cb.dispatchOps([{ op: 'tableCol', id: block.id, action: 'move', index: selection.index, toIndex: selection.index - 1, noteWord: { key: '@jinshu.note.columnBack' } }]);
                sel.set({ kind: 'col', index: selection.index - 1 });
              }}
            />
          )}
          {selection.index < nCols - 1 && (
            <Button
              label={jinshuText('@jinshu.editor.block.moveRight')}
              size="sm"
              variant="ghost"
              onClick={() => {
                cb.dispatchOps([{ op: 'tableCol', id: block.id, action: 'move', index: selection.index, toIndex: selection.index + 1, noteWord: { key: '@jinshu.note.columnForward' } }]);
                sel.set({ kind: 'col', index: selection.index + 1 });
              }}
            />
          )}
          {nCols > 1 && (
            <>
              <view aria-hidden className={b.table.toolbarSeparator} />
              <view onPointerEnter={() => dangerHover.set(true)} onPointerLeave={() => dangerHover.set(false)}>
                <Button
                  label={jinshuText('@jinshu.editor.block.deleteColumn')}
                  startIcon="delete"
                  size="sm"
                  variant={isDanger ? 'destructive' : 'ghost'}
                  onClick={() => {
                    dangerHover.set(false);
                    cb.dispatchOps([{ op: 'tableCol', id: block.id, action: 'delete', index: selection.index, noteWord: { key: '@jinshu.note.columnDelete' } }]);
                    sel.set(null);
                  }}
                />
              </view>
            </>
          )}
        </view>
      </scroll>
    ) : (
      <scroll className={b.table.toolbar}>
        <view className={b.table.toolbarRow}>
          <text className={b.table.toolbarLabel}>{selection.index === -1 ? jinshuText('@jinshu.editor.block.headerRow') : jinshuText('@jinshu.editor.block.row', { index: selection.index + 1 })}</text>
          {selection.index >= 0 && (
            <Button label={jinshuText('@jinshu.editor.block.insertAbove')} size="sm" variant="ghost" onClick={() => { insertRow(selection.index); sel.set({ kind: 'row', index: selection.index + 1 }); }} />
          )}
          <Button label={jinshuText('@jinshu.editor.block.insertBelow')} size="sm" variant="ghost" onClick={() => insertRow(selection.index + 1)} />
          {selection.index > 0 && (
            <Button
              label={jinshuText('@jinshu.editor.block.moveUp')}
              size="sm"
              variant="ghost"
              onClick={() => {
                cb.dispatchOps([{ op: 'tableRow', id: block.id, action: 'move', rowId: bodyRows[selection.index].id, before: bodyRows[selection.index - 1].id, noteWord: { key: '@jinshu.note.rowUp' } }]);
                sel.set({ kind: 'row', index: selection.index - 1 });
              }}
            />
          )}
          {selection.index >= 0 && selection.index < nRows - 1 && (
            <Button
              label={jinshuText('@jinshu.editor.block.moveDown')}
              size="sm"
              variant="ghost"
              onClick={() => {
                cb.dispatchOps([{ op: 'tableRow', id: block.id, action: 'move', rowId: bodyRows[selection.index].id, after: bodyRows[selection.index + 1].id, noteWord: { key: '@jinshu.note.rowDown' } }]);
                sel.set({ kind: 'row', index: selection.index + 1 });
              }}
            />
          )}
          {selection.index >= 0 && nRows > 1 && (
            <>
              <view aria-hidden className={b.table.toolbarSeparator} />
              <view onPointerEnter={() => dangerHover.set(true)} onPointerLeave={() => dangerHover.set(false)}>
                <Button
                  label={jinshuText('@jinshu.editor.block.deleteRow')}
                  startIcon="delete"
                  size="sm"
                  variant={isDanger ? 'destructive' : 'ghost'}
                  onClick={() => {
                    dangerHover.set(false);
                    cb.dispatchOps([{ op: 'tableRow', id: block.id, action: 'delete', rowId: bodyRows[selection.index].id, noteWord: { key: '@jinshu.note.rowDelete' } }]);
                    sel.set(null);
                  }}
                />
              </view>
            </>
          )}
        </view>
      </scroll>
    );

  const firstRow = headerRow ? -1 : 0;

  // The reader's cell, with the editor's handles laid over its padding so nothing moves.
  const renderCell = (row: number, col: number) => {
    const isHeader = row === -1;
    const isEd = pos?.row === row && pos?.col === col;
    const inSel = selection !== null && ((selection.kind === 'col' && selection.index === col) || (selection.kind === 'row' && selection.index === row));
    const al = block.align?.[col];
    const runs = cellRuns(row, col);
    const textClass = isHeader ? r.table.cell.header : r.table.cell.text;
    return (
      <view
        key={`c-${row}-${col}`}
        style={tableColumnFlex(block, col)}
        className={cx(r.table.cell.box, inSel && (isDanger ? b.table.cell.doomed : b.table.cell.selected), isEd && b.table.cell.editing)}
      >
        {isEd ? (
          <textarea
            aria-label={isHeader ? jinshuText('@jinshu.editor.block.headerCell', { column: col + 1 }) : jinshuText('@jinshu.editor.block.cell', { row: row + 1, column: col + 1 })}
            value={cellDraft.value}
            autoFocus
            rows={1}
            maxRows={6}
            onInput={v => { cellDraft.set(v); cb.onTyping?.(); }}
            onBlur={() => {
              commitCell(row, col, cellDraft.value);
              if (activePos.value?.row === row && activePos.value?.col === col) activePos.set(null);
            }}
            onKeyDown={{
              Enter: () => {
                commitCell(row, col, cellDraft.value);
                if (row + 1 < nRows) focusCell(row + 1, col);
                else {
                  insertRow(nRows);
                  cb.onFocusBlock(block.id);
                  activePos.set({ row: nRows, col });
                  cellDraft.set('');
                }
                return true;
              },
              ArrowUp: () => {
                if (row === -1) return false;
                commitCell(row, col, cellDraft.value);
                focusCell(row - 1, col);
                return true;
              },
              ArrowDown: () => {
                if (row + 1 >= nRows) return false;
                commitCell(row, col, cellDraft.value);
                focusCell(row + 1, col);
                return true;
              },
              Escape: () => {
                commitCell(row, col, cellDraft.value);
                activePos.set(null);
                return true;
              },
            }}
            className={isHeader ? b.table.input.header : b.table.input.text}
          />
        ) : (
          <button aria-label={isHeader ? jinshuText('@jinshu.editor.block.editHeaderCell', { column: col + 1 }) : jinshuText('@jinshu.editor.block.editCell', { row: row + 1, column: col + 1 })} onClick={() => focusCell(row, col)} className={b.table.cellButton}>
            <text className={cx(textClass, runs.length === 0 && b.table.cellEmpty)} style={al ? { textAlign: al } : undefined}>
              {runs.length > 0 ? renderInlineRuns(runs, { blockId: block.id }) : isHeader ? jinshuText('@jinshu.editor.block.headerPlaceholder') : '\u200B'}
            </text>
          </button>
        )}
        {row === firstRow && (
          <button aria-label={jinshuText('@jinshu.editor.block.selectColumn', { index: col + 1 })} onClick={() => selectLine('col', col)} className={b.table.colHandle}>
            <view className={cx(b.table.handle.base, b.table.handle.col, selection?.kind === 'col' && selection.index === col ? (isDanger ? b.table.handle.doomed : b.table.handle.selected) : chrome ? b.table.handle.idle : b.table.handle.hidden)} />
          </button>
        )}
        {col === 0 && (
          <button
            aria-label={isHeader ? jinshuText('@jinshu.editor.block.selectHeaderRow') : jinshuText('@jinshu.editor.block.selectRow', { index: row + 1 })}
            onClick={() => selectLine('row', row)}
            className={b.table.rowHandle}
          >
            <view className={cx(b.table.handle.base, b.table.handle.row, selection?.kind === 'row' && selection.index === row ? (isDanger ? b.table.handle.doomed : b.table.handle.selected) : chrome ? b.table.handle.idle : b.table.handle.hidden)} />
          </button>
        )}
      </view>
    );
  };

  return (
    <view
      className={b.table.root}
      onPointerEnter={touch ? undefined : () => hovered.set(true)}
      onPointerLeave={touch ? undefined : () => hovered.set(false)}
    >
      {lineToolbar && <view className={b.table.toolbarDock}>{lineToolbar}</view>}
      <TableFrame table={block} testId={cb.testScope && `${cb.testScope}-table-editor-${block.id}`}>
        {headerRow && <view className={r.table.headerRow}>{headerRow.cells.map((_, cIdx) => renderCell(-1, cIdx))}</view>}
        {bodyRows.map((row, rIdx) => (
          <view key={`r-${row.id}`} className={r.table.row}>
            {row.cells.map((_, cIdx) => renderCell(rIdx, cIdx))}
          </view>
        ))}
      </TableFrame>
    </view>
  );
}

// ---------------------------------------------------------------------------
// Columns
// ---------------------------------------------------------------------------

// Where the caret is in a column's open field, by its draft cell; read on Enter.
const columnCarets = new WeakMap<object, TextRange & { readonly id: string }>();

/**
 * `columns` with every child in its own editor: a paragraph or heading opens in place and Enter starts a
 * paragraph below it. A bar over the block switches 2 and 3 columns, sets widths, and moves, inserts or
 * removes the block last touched in a column.
 */
export function ColumnsBlockEditor({
  block,
  cb,
  renderFallback,
}: {
  readonly block: ColumnsBlock;
  readonly cb: BlockEditorCallbacks;
  readonly renderFallback: (child: JinshuBlock) => JSX.Element;
}) {
  const { env } = useView();
  const activeChildId = useLocal<string | null>(`ed-col-act-${block.id}`, null);
  const childDraft = useLocal<string>(`ed-col-txt-${block.id}`, '');
  // The child last touched, which the bar's block actions apply to.
  const focusedId = useLocal<string | null>(`ed-col-focus-${block.id}`, null);
  const inspectingId = useLocal<string | null>(`ed-col-insp-${block.id}`, null);
  const hovered = useLocal(`ed-col-hover-${block.id}`, false);
  const shouldStack = env.sizeClass === 'compact' || env.textScale >= 1.35;
  const touch = env.pointer === 'coarse';

  const located = (id: string | null) => {
    if (id === null) return null;
    for (const col of block.columns) {
      const index = col.blocks.findIndex(child => child.id === id);
      if (index >= 0) return { col, index, child: col.blocks[index] };
    }
    return null;
  };
  const focused = located(focusedId.value);
  // A phone shows the bar only once a child is touched, so it never sits over the text above.
  const showBar = activeChildId.value !== null || (focused !== null && touch) || (!touch && hovered.value);
  const wideIndex = (() => {
    const w = block.widths;
    if (!w || w.length !== block.columns.length) return null;
    const max = Math.max(...w);
    return w.filter(x => x === max).length === 1 && w.some(x => x !== max) ? w.indexOf(max) : null;
  })();

  const inlineOps = (child: JinshuBlock, text: string): AnyJinshuOp[] => {
    if (child.type !== 'paragraph' && child.type !== 'heading') return [];
    if (inlinesToShorthand(child.runs, cb.format) === text) return [];
    const runs = shorthandToInlines(text, cb.format, { inHeading: child.type === 'heading', existingAtoms: atomsIn(child.runs) });
    return [{ op: 'replace', id: child.id, blocks: [child.type === 'heading' ? { id: child.id, type: 'heading', level: child.level, runs } : { id: child.id, type: 'paragraph', runs }] }];
  };
  const saveChild = () => {
    const at = located(activeChildId.value);
    if (!at) return;
    const ops = inlineOps(at.child, childDraft.value);
    if (ops.length > 0) cb.dispatchOps(ops);
  };
  const closeChild = () => {
    saveChild();
    activeChildId.set(null);
    cb.onBlurBlock();
  };
  cb.holdOpen?.(block.id, activeChildId.value === null ? null : closeChild, saveChild);

  const touchChild = (id: string) => {
    focusedId.set(id);
    cb.onFocusBlock(id);
  };
  const openParagraph = (id: string, text: string) => {
    touchChild(id);
    activeChildId.set(id);
    childDraft.set(text);
  };
  // Child editors report focus through here, so the bar knows which block it acts on.
  const childCb: BlockEditorCallbacks = {
    ...cb,
    onFocusBlock: touchChild,
    onEditParagraph: id => openParagraph(id, ''),
  };

  const splitChild = (child: JinshuBlock) => {
    const text = childDraft.value;
    const held = columnCarets.get(childDraft);
    const range = held && held.id === child.id && held.end <= text.length ? held : { start: text.length, end: text.length };
    const [before, after] = splitAt(text, range, SHORTHAND_MARKS);
    const next = paragraph(after ? shorthandToInlines(after, cb.format, {}) : '');
    cb.dispatchOps([...inlineOps(child, before), { op: 'insert', at: { after: child.id }, blocks: [next] }]);
    openParagraph(next.id, after);
    childDraft.setSelection(0, 0);
    columnCarets.set(childDraft, { id: next.id, start: 0, end: 0 });
  };
  const insertBelow = (afterId: string) => {
    saveChild();
    const next = paragraph('');
    cb.dispatchOps([{ op: 'insert', at: { after: afterId }, blocks: [next], noteWord: { key: '@jinshu.note.insert', args: { kind: 'paragraph' } } }]);
    openParagraph(next.id, '');
  };
  const removeChild = (id: string) => {
    if (activeChildId.value === id) activeChildId.set(null);
    focusedId.set(null);
    cb.dispatchOps([{ op: 'delete', id, noteWord: { key: '@jinshu.note.delete' } }]);
    cb.onBlurBlock();
  };
  const moveChild = (id: string, to: { readonly before: string } | { readonly after: string }, noteKey: '@jinshu.note.moveUp' | '@jinshu.note.moveDown') => {
    saveChild();
    cb.dispatchOps([{ op: 'move', id, to, noteWord: { key: noteKey } }]);
  };
  const setColumnCount = (count: number) => {
    if (count === block.columns.length) return;
    saveChild();
    // Going down to two, the third column's blocks join the second, so nothing is lost.
    const cols =
      count > block.columns.length
        ? [...block.columns, { id: newBlockId('col'), blocks: [paragraph('')] }]
        : [block.columns[0], { ...block.columns[1], blocks: [...block.columns[1].blocks, ...block.columns.slice(2).flatMap(col => col.blocks)] }];
    const { widths: _widths, ...rest } = block;
    cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...rest, columns: cols }], noteWord: { key: '@jinshu.note.columnCount', args: { count } } }]);
  };
  const setWide = (index: number | null) =>
    cb.dispatchOps([{ op: 'update', id: block.id, attrs: { widths: index === null ? null : block.columns.map((_, i) => (i === index ? 2 : 1)) }, noteWord: { key: '@jinshu.note.columnWidths' } }]);

  const renderChild = (child: JinshuBlock): JSX.Element => {
    switch (child.type) {
      case 'list': return <ListBlockEditor block={child} cb={childCb} />;
      case 'callout': return <CalloutBlockEditor block={child} cb={childCb} />;
      case 'quote': return <QuoteBlockEditor block={child} cb={childCb} />;
      case 'details': return <DetailsBlockEditor block={child} cb={childCb} />;
      case 'code': return <CodeBlockEditor block={child} cb={childCb} />;
      case 'table': return <TableBlockEditor block={child} cb={childCb} />;
      case 'math': return <MathBlockEditor block={child} cb={childCb} preview={renderFallback(child)} />;
      case 'paragraph':
      case 'heading':
        if (activeChildId.value === child.id) {
          return (
            <textarea
              aria-label={jinshuText('@jinshu.editor.block.editInColumn', { type: child.type })}
              bind={childDraft}
              autoFocus
              rows={1}
              maxRows={18}
              onInput={() => cb.onTyping?.()}
              onSelectionChange={sel => columnCarets.set(childDraft, { id: child.id, start: sel.start, end: sel.end })}
              onBlur={() => {
                if (activeChildId.value !== child.id) return;
                closeChild();
              }}
              onKeyDown={{
                Enter: () => {
                  splitChild(child);
                  return true;
                },
                Escape: () => {
                  closeChild();
                  return true;
                },
              }}
              className={child.type === 'heading' ? cx(b.columns.input, r.heading.base, r.heading.level[child.level]) : cx(b.columns.input, r.paragraph.base)}
            />
          );
        }
        return (
          <view className={b.columns.child}>
            {child.type === 'paragraph' && child.runs.length === 0 ? (
              <text className={b.columns.placeholder}>{jinshuText('@jinshu.editor.block.columnPlaceholder')}</text>
            ) : (
              renderFallback(child)
            )}
            <button
              aria-label={jinshuText('@jinshu.editor.block.editColumnBlock', { id: child.id })}
              onClick={() => openParagraph(child.id, inlinesToShorthand(child.runs, cb.format))}
              className={b.columns.childButton}
            />
          </view>
        );
      case 'image':
      case 'video':
      case 'audio':
      case 'file':
        return (
          <view className={b.columns.media}>
            {renderFallback(child)}
            {(touch || hovered.value || inspectingId.value === child.id) && (
              <view className={b.columns.mediaTool}>
                <Tooltip content={jinshuText('@jinshu.editor.blockMenu.properties')} placement="bottom"><IconButton
                  label={jinshuText('@jinshu.editor.blockMenu.properties')}
                  icon="edit"
                  size="sm"
                  variant={inspectingId.value === child.id ? 'tonal' : 'secondary'}
                  onClick={() => {
                    touchChild(child.id);
                    inspectingId.set(inspectingId.value === child.id ? null : child.id);
                  }}
                /></Tooltip>
              </view>
            )}
            {inspectingId.value === child.id && (child.type === 'image' ? <ImageBlockInspector block={child} cb={childCb} /> : <MediaBlockInspector block={child} cb={childCb} />)}
          </view>
        );
      default:
        return renderFallback(child);
    }
  };

  const bar = showBar && (
    <view className={b.table.toolbarDock}>
      <scroll className={b.table.toolbar}>
        <view role="toolbar" aria-label={jinshuText('@jinshu.blockType', { type: 'columns' })} className={b.table.toolbarRow}>
          {[2, 3].map(count => (
            <Button
              key={String(count)}
              label={jinshuText('@jinshu.editor.block.columnCount', { count })}
              size="sm"
              variant={block.columns.length === count ? 'tonal' : 'ghost'}
              onClick={() => setColumnCount(count)}
            />
          ))}
          {holds(cb, 'columns', 'widths') && (
            <>
              <view aria-hidden className={b.table.toolbarSeparator} />
              <Button label={jinshuText('@jinshu.editor.block.columnsEqual')} size="sm" variant={wideIndex === null ? 'tonal' : 'ghost'} onClick={() => setWide(null)} />
              {block.columns.map((col, i) => (
                <Button key={col.id} label={jinshuText('@jinshu.editor.block.columnWider', { index: i + 1 })} size="sm" variant={wideIndex === i ? 'tonal' : 'ghost'} onClick={() => setWide(i)} />
              ))}
            </>
          )}
          {focused && (
            <>
              <view aria-hidden className={b.table.toolbarSeparator} />
              <Button label={jinshuText('@jinshu.editor.block.insertBelow')} startIcon="add" size="sm" variant="ghost" onClick={() => insertBelow(focused.child.id)} />
              {focused.index > 0 && (
                <Button label={jinshuText('@jinshu.editor.block.moveUp')} size="sm" variant="ghost" onClick={() => moveChild(focused.child.id, { before: focused.col.blocks[focused.index - 1].id }, '@jinshu.note.moveUp')} />
              )}
              {focused.index < focused.col.blocks.length - 1 && (
                <Button label={jinshuText('@jinshu.editor.block.moveDown')} size="sm" variant="ghost" onClick={() => moveChild(focused.child.id, { after: focused.col.blocks[focused.index + 1].id }, '@jinshu.note.moveDown')} />
              )}
              {/* A column keeps at least one block. */}
              {focused.col.blocks.length > 1 && (
                <Button label={jinshuText('@jinshu.editor.block.removeItem')} startIcon="delete" size="sm" variant="ghost" onClick={() => removeChild(focused.child.id)} />
              )}
            </>
          )}
        </view>
      </scroll>
    </view>
  );

  return (
    <view
      className={b.columns.root}
      onPointerEnter={touch ? undefined : () => hovered.set(true)}
      onPointerLeave={touch ? undefined : () => hovered.set(false)}
    >
      {bar}
      <view className={shouldStack ? b.columns.stacked : b.columns.side}>
        {block.columns.map((col, colIdx) => (
          <view key={col.id} className={b.columns.column} style={shouldStack ? undefined : { flexGrow: block.widths?.[colIdx] ?? 1, flexBasis: 0 }}>
            {col.blocks.map(child => (
              <view key={child.id} className={cx(b.columns.slot, showBar && focused?.child.id === child.id && b.columns.slotFocused)}>
                {renderChild(child)}
              </view>
            ))}
          </view>
        ))}
      </view>
    </view>
  );
}

/** A formula in a column: the reader's formula until pressed, then its LaTeX under a live preview. */
export function MathBlockEditor({ block, cb, preview }: { readonly block: MathBlock; readonly cb: BlockEditorCallbacks; readonly preview: JSX.Element }) {
  const editing = useLocal(`ed-math-${block.id}`, false);
  const draft = useLocal(`ed-math-t-${block.id}`, block.latex);
  const save = () => {
    if (draft.value !== block.latex) cb.dispatchOps([{ op: 'update', id: block.id, attrs: { latex: draft.value } }]);
  };
  const commit = () => {
    save();
    editing.set(false);
    cb.onBlurBlock();
  };
  cb.holdOpen?.(block.id, editing.value ? commit : null, save);

  if (!editing.value) {
    return (
      <view className={b.columns.child}>
        {preview}
        <button
          aria-label={jinshuText('@jinshu.editor.block.editColumnBlock', { id: block.id })}
          onClick={() => {
            cb.onFocusBlock(block.id);
            draft.set(block.latex);
            editing.set(true);
          }}
          className={b.columns.childButton}
        />
      </view>
    );
  }
  return (
    <view className={m.base}>
      <scroll className={m.scroll}>
        <view className={m.preview}>
          <MathFormulaView latex={draft.value || block.latex} />
        </view>
      </scroll>
      <view className={m.source}>
        <textarea
          aria-label={jinshuText('@jinshu.editor.edit.source', { type: 'math' })}
          value={draft.value}
          autoFocus
          rows={1}
          maxRows={6}
          onInput={v => { draft.set(v); cb.onTyping?.(); }}
          onBlur={save}
          onKeyDown={{ Escape: () => { commit(); return true; } }}
          className={m.field}
        />
        <view className={m.footer}>
          <text className={m.hint}>{jinshuText('@jinshu.editor.math.hint')}</text>
          <Button label={jinshuText('@jinshu.editor.done')} size="sm" variant="tonal" onClick={commit} />
        </view>
      </view>
    </view>
  );
}

// ---------------------------------------------------------------------------
// Inspectors
// ---------------------------------------------------------------------------

/**
 * A field's draft that follows the block until the person types: a change made elsewhere meanwhile
 * shows, and leaving the field does not write the old text back over it.
 */
function useFieldDraft(name: string, current: string): { readonly value: string; readonly isEdited: boolean; set(next: string): void; settle(): void } {
  const draft = useLocal(name, { seed: current, text: current });
  const { seed, text } = draft.value;
  const isEdited = text !== seed;
  return {
    value: isEdited ? text : current,
    isEdited,
    set: next => draft.set({ seed: current, text: next }),
    // Once its text is sent, the field follows the document again.
    settle: () => draft.set({ seed: text, text }),
  };
}

/** Inline inspector for Image blocks (alt, caption, display size). */
export function ImageBlockInspector({ block, cb }: { readonly block: ImageBlock; readonly cb: BlockEditorCallbacks }) {
  const alt = block.alt ?? '';
  const caption = block.caption ? inlinesToShorthand(block.caption, cb.format) : '';
  const altDraft = useFieldDraft(`ed-img-alt-${block.id}`, alt);
  const capDraft = useFieldDraft(`ed-img-cap-${block.id}`, caption);

  const commit = () => {
    if (altDraft.value === alt && capDraft.value === caption) return;
    cb.dispatchOps([
      {
        op: 'update',
        id: block.id,
        attrs: {
          // An empty alt reads back as none in Markdown, so none it is.
          alt: altDraft.value || null,
          ...(holds(cb, 'image', 'caption')
            ? { caption: capDraft.value.trim() ? shorthandToInlines(capDraft.value, cb.format, { existingAtoms: atomsIn(block.caption) }) : null }
            : {}),
        },
        noteWord: { key: '@jinshu.note.imageProps' },
      },
    ]);
    altDraft.settle();
    capDraft.settle();
  };

  return (
    <view className={b.inspector.root}>
      {holds(cb, 'image', 'size') && (
        <view className={b.inspector.choices}>
          <text className={b.inspector.label}>{jinshuText('@jinshu.editor.block.imageSizeLabel')}</text>
          {(['small', 'medium', 'full'] as ImageSize[]).map(sz => (
            <Button
              key={sz}
              label={jinshuText('@jinshu.editor.block.imageSize', { size: sz })}
              size="sm"
              variant={(block.size ?? 'full') === sz ? 'tonal' : 'ghost'}
              onClick={() => cb.dispatchOps([{ op: 'update', id: block.id, attrs: { size: sz === 'full' ? null : sz } }])}
            />
          ))}
        </view>
      )}
      <input aria-label={jinshuText('@jinshu.editor.block.imageAlt')} value={altDraft.value} placeholder={jinshuText('@jinshu.editor.block.imageAlt')} onInput={v => altDraft.set(v)} onBlur={commit} className={b.inspector.field} />
      {holds(cb, 'image', 'caption') && (
        <input aria-label={jinshuText('@jinshu.editor.block.imageCaption')} value={capDraft.value} placeholder={jinshuText('@jinshu.editor.block.imageCaptionPlaceholder')} onInput={v => capDraft.set(v)} onBlur={commit} className={b.inspector.field} />
      )}
    </view>
  );
}

/** Inline inspector for Video, Audio and File blocks. */
export function MediaBlockInspector({ block, cb }: { readonly block: VideoBlock | AudioBlock | FileBlock; readonly cb: BlockEditorCallbacks }) {
  const primary =
    block.type === 'video' ? (block.caption ? inlinesToShorthand(block.caption, cb.format) : '') : block.type === 'audio' ? block.title ?? '' : block.name;
  const secondary = block.type === 'file' ? block.description ?? '' : '';
  const primaryDraft = useFieldDraft(`ed-media-p-${block.id}`, primary);
  const secondaryDraft = useFieldDraft(`ed-media-s-${block.id}`, secondary);

  const commit = () => {
    if (primaryDraft.value === primary && secondaryDraft.value === secondary) return;
    const attrs: Record<string, unknown> =
      block.type === 'video'
        ? { caption: primaryDraft.value.trim() ? shorthandToInlines(primaryDraft.value, cb.format, { existingAtoms: atomsIn(block.caption) }) : null }
        : block.type === 'audio'
          ? { title: primaryDraft.value.trim() || null }
          : { name: primaryDraft.value.trim() || block.name, description: secondaryDraft.value.trim() || null };
    cb.dispatchOps([{ op: 'update', id: block.id, attrs, noteWord: { key: '@jinshu.note.mediaProps' } }]);
    primaryDraft.settle();
    secondaryDraft.settle();
  };

  return (
    <view className={b.inspector.root}>
      <input
        aria-label={jinshuText('@jinshu.editor.block.mediaName', { type: block.type })}
        value={primaryDraft.value}
        placeholder={jinshuText('@jinshu.editor.block.mediaName', { type: block.type })}
        onInput={v => primaryDraft.set(v)}
        onBlur={commit}
        className={b.inspector.field}
      />
      {block.type === 'file' && (
        <>
          <input aria-label={jinshuText('@jinshu.editor.block.fileDescription')} value={secondaryDraft.value} placeholder={jinshuText('@jinshu.editor.block.fileDescriptionPlaceholder')} onInput={v => secondaryDraft.set(v)} onBlur={commit} className={b.inspector.field} />
          <view className={b.inspector.choices}>
            <text className={b.inspector.label}>{jinshuText('@jinshu.editor.block.fileDisplayLabel')}</text>
            {(['attachment', 'inline'] as const).map(d => (
              <Button
                key={d}
                label={jinshuText('@jinshu.editor.block.fileDisplay', { display: d })}
                size="sm"
                variant={(block.display ?? 'attachment') === d ? 'tonal' : 'ghost'}
                onClick={() => cb.dispatchOps([{ op: 'update', id: block.id, attrs: { display: d === 'attachment' ? null : d } }])}
              />
            ))}
          </view>
        </>
      )}
      <text className={b.inspector.src}>{block.src}</text>
    </view>
  );
}
