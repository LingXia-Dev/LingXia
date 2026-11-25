import { cx, useLocal, useView } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Icon } from '@liulidesign/core/Icon';
import { IconButton } from '@liulidesign/core/IconButton';
import { editorIcons } from './editorIcons';
import { newBlockId, paragraph, plainText, tableColumnCount } from '../schema/helpers';
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
  JinshuBlock,
  ListBlock,
  ListStyle,
  ParagraphBlock,
  QuoteBlock,
  TableAlign,
  TableBlock,
  VideoBlock,
} from '../schema/types';
import { renderInlineRuns } from '../internal/InlineRuns';
import { CodeFrame, CodeLineNumbers, TableFrame, tableColumnFlex } from '../internal/Blocks';
import { jinshuViewRecipe as r } from '../JinshuView/JinshuView.recipe';
import { jinshuText } from '../i18n/text';
import { blockEditorsRecipe as b } from './BlockEditors.recipe';
import { atomsIn, inlinesToShorthand, shorthandToInlines } from './inlineMarkdown';

const TICK_PATH = [
  { d: 'M3.5 8.5l3 3 6-7', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' },
] as const;

export interface BlockEditorCallbacks {
  /** The editor's test id, under which the block editors' own test ids hang. */
  readonly testScope?: string;
  readonly dispatchOps: (ops: readonly AnyJinshuOp[]) => void;
  readonly onFocusBlock: (blockId: string) => void;
  readonly onBlurBlock: () => void;
  /** Inserts an empty paragraph after `afterId` and puts the caret in it (leaving a list or container). */
  readonly onCreateParagraphAfter?: (afterId: string) => void;
}

// ---------------------------------------------------------------------------
// Container bodies: every paragraph child is editable, blank line = new paragraph
// ---------------------------------------------------------------------------

function isTextChild(b: JinshuBlock): b is ParagraphBlock {
  return b.type === 'paragraph';
}

/** Paragraph children joined by blank lines; non-paragraph children are kept as-is and counted. */
function containerBodyText(blocks: readonly JinshuBlock[]): { text: string; kept: number } {
  const paras = blocks.filter(isTextChild);
  return { text: paras.map(p => inlinesToShorthand(p.runs)).join('\n\n'), kept: blocks.length - paras.length };
}

/** Rebuilds container children: paragraphs from blank-line-separated text (ids reused in order), other children kept in place. */
function rebuildContainerBody(blocks: readonly JinshuBlock[], text: string): JinshuBlock[] {
  const parts = text
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/)
    .map(s => s.replace(/^\n+|\n+$/g, ''))
    .filter(s => s.length > 0);
  const oldParas = blocks.filter(isTextChild);
  const existingAtoms = oldParas.flatMap(p => atomsIn(p.runs));
  const newParas: JinshuBlock[] = (parts.length > 0 ? parts : ['']).map((part, idx) =>
    paragraph(shorthandToInlines(part, { existingAtoms }), oldParas[idx]?.id ?? newBlockId('p')),
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
  const activeItemId = useLocal<string | null>(`ed-li-${block.id}`, null);
  const itemDraft = useLocal<string>(`ed-li-txt-${block.id}`, '');
  const start = block.start ?? 1;

  const commitItem = (itemId: string, text: string) => {
    const item = block.items.find(it => it.id === itemId);
    if (!item || inlinesToShorthand(item.runs) === text) return;
    cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, items: block.items.map(it => (it.id === itemId ? { ...it, runs: shorthandToInlines(text, { existingAtoms: atomsIn(it.runs) }) } : it)) }] }]);
  };

  const focusItem = (idx: number) => {
    const target = block.items[idx];
    if (!target) return;
    activeItemId.set(target.id);
    itemDraft.set(inlinesToShorthand(target.runs));
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
                  <input
                    aria-label={jinshuText('@jinshu.editor.block.editItem', { index: idx + 1 })}
                    value={itemDraft.value}
                    autoFocus
                    placeholder={jinshuText('@jinshu.editor.block.itemPlaceholder')}
                    onInput={v => itemDraft.set(v)}
                    onBlur={() => {
                      commitItem(item.id, itemDraft.value);
                      if (activeItemId.value === item.id) activeItemId.set(null);
                      cb.onBlurBlock();
                    }}
                    onKeyDown={{
                      Enter: () => {
                        // Empty last item: leave the list for a fresh paragraph (Typora / Notion behaviour).
                        if (itemDraft.value.trim() === '' && block.items.length > 1) {
                          cb.dispatchOps([{ op: 'delete', id: item.id, noteWord: { key: '@jinshu.note.listExit' } }]);
                          activeItemId.set(null);
                          cb.onCreateParagraphAfter?.(block.id);
                          return;
                        }
                        const newId = newBlockId('li');
                        const nextItems = [...block.items];
                        nextItems[idx] = { ...item, runs: shorthandToInlines(itemDraft.value, { existingAtoms: atomsIn(item.runs) }) };
                        nextItems.splice(idx + 1, 0, { id: newId, runs: [] });
                        cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, items: nextItems }] }]);
                        activeItemId.set(newId);
                        itemDraft.set('');
                      },
                      ArrowUp: () => {
                        if (idx === 0) return;
                        commitItem(item.id, itemDraft.value);
                        focusItem(idx - 1);
                      },
                      ArrowDown: () => {
                        if (idx >= block.items.length - 1) return;
                        commitItem(item.id, itemDraft.value);
                        focusItem(idx + 1);
                      },
                      Escape: () => {
                        commitItem(item.id, itemDraft.value);
                        activeItemId.set(null);
                        cb.onBlurBlock();
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
                <Button
                  label={jinshuText('@jinshu.editor.block.removeItem')}
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    cb.dispatchOps([{ op: 'delete', id: item.id, noteWord: { key: '@jinshu.note.listItemDelete' } }]);
                    activeItemId.set(null);
                  }}
                />
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
  const editing = useLocal(`ed-co-${block.id}`, false);
  const titleDraft = useLocal(`ed-co-t-${block.id}`, '');
  const bodyDraft = useLocal(`ed-co-b-${block.id}`, '');
  const body = containerBodyText(block.blocks);

  const openEdit = () => {
    cb.onFocusBlock(block.id);
    titleDraft.set(block.title ? inlinesToShorthand(block.title) : '');
    bodyDraft.set(body.text);
    editing.set(true);
  };

  const commit = () => {
    const nextTitle = titleDraft.value.trim() ? shorthandToInlines(titleDraft.value, { existingAtoms: atomsIn(block.title) }) : undefined;
    const sameTitle = (block.title ? inlinesToShorthand(block.title) : '') === titleDraft.value.trim();
    if (!sameTitle || bodyDraft.value !== body.text) {
      const next: Record<string, unknown> = { ...block, blocks: rebuildContainerBody(block.blocks, bodyDraft.value) };
      if (nextTitle) next.title = nextTitle;
      else delete next.title;
      cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [next as unknown as CalloutBlock], noteWord: { key: '@jinshu.note.editCallout' } }]);
    }
    editing.set(false);
    cb.onBlurBlock();
  };

  if (!editing.value) {
    return (
      <view className={cx(r.callout.base, r.callout.tone[block.tone], b.callout.preview)}>
        {block.title && block.title.length > 0 && <text className={r.callout.title[block.tone]}>{renderInlineRuns(block.title, { blockId: block.id })}</text>}
        <ContainerPreview blocks={block.blocks} />
        <button aria-label={jinshuText('@jinshu.editor.block.editCallout', { id: block.id })} onClick={openEdit} className={b.callout.previewButton} />
      </view>
    );
  }

  return (
    <view className={cx(r.callout.base, r.callout.tone[block.tone], b.callout.editing)}>
      <view className={b.callout.header}>
        <input
          aria-label={jinshuText('@jinshu.editor.block.calloutTitle')}
          value={titleDraft.value}
          placeholder={jinshuText('@jinshu.editor.block.calloutTitlePlaceholder')}
          onInput={v => titleDraft.set(v)}
          className={cx(b.callout.titleInput, r.callout.title[block.tone])}
        />
        <view className={b.callout.tones}>
          {(['info', 'tip', 'warning', 'danger'] as CalloutTone[]).map(tone => (
            <Button
              key={tone}
              label={jinshuText('@jinshu.editor.block.calloutTone', { tone })}
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
        onInput={v => bodyDraft.set(v)}
        onKeyDown={{ Escape: commit }}
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
  const editing = useLocal(`ed-q-${block.id}`, false);
  const draft = useLocal(`ed-q-t-${block.id}`, '');
  const body = containerBodyText(block.blocks);

  const commit = () => {
    if (draft.value !== body.text) {
      cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, blocks: rebuildContainerBody(block.blocks, draft.value) }], noteWord: { key: '@jinshu.note.editQuote' } }]);
    }
    editing.set(false);
    cb.onBlurBlock();
  };

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
        onInput={v => draft.set(v)}
        onBlur={commit}
        onKeyDown={{ Escape: commit }}
        className={cx(b.quote.body, r.paragraph.base, b.quote.bodyText)}
      />
      <KeptChildrenNote count={body.kept} />
    </view>
  );
}

/** Direct-manipulation editor for Details blocks (summary + every body paragraph). */
export function DetailsBlockEditor({ block, cb }: { readonly block: DetailsBlock; readonly cb: BlockEditorCallbacks }) {
  const open = useLocal(`ed-det-open-${block.id}`, block.defaultOpen === true);
  const summaryDraft = useLocal(`ed-det-s-${block.id}`, inlinesToShorthand(block.summary));
  const body = containerBodyText(block.blocks);
  const bodyDraft = useLocal(`ed-det-b-${block.id}`, body.text);

  const commit = () => {
    if (summaryDraft.value === inlinesToShorthand(block.summary) && bodyDraft.value === body.text) return;
    cb.dispatchOps([
      {
        op: 'replace',
        id: block.id,
        blocks: [{ ...block, summary: shorthandToInlines(summaryDraft.value, { existingAtoms: atomsIn(block.summary) }), blocks: rebuildContainerBody(block.blocks, bodyDraft.value) }],
        noteWord: { key: '@jinshu.note.editDetails' },
      },
    ]);
  };

  // The reader's geometry: the summary row with its chevron at the end; whether it starts open is a block-menu choice.
  return (
    <view>
      <view className={b.details.header}>
        <input
          aria-label={jinshuText('@jinshu.editor.block.detailsSummary')}
          value={summaryDraft.value}
          placeholder={jinshuText('@jinshu.editor.block.detailsSummaryPlaceholder')}
          onInput={v => summaryDraft.set(v)}
          onBlur={commit}
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
            onInput={v => bodyDraft.set(v)}
            onBlur={commit}
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
  const editing = useLocal(`ed-code-${block.id}`, false);
  const langDraft = useLocal(`ed-code-l-${block.id}`, block.language);
  const fileDraft = useLocal(`ed-code-f-${block.id}`, block.filename ?? '');
  const codeDraft = useLocal(`ed-code-c-${block.id}`, block.code);

  const commit = () => {
    cb.dispatchOps([
      {
        op: 'update',
        id: block.id,
        attrs: { language: langDraft.value.trim() || 'text', filename: fileDraft.value.trim() || null, code: codeDraft.value },
        noteWord: { key: '@jinshu.note.editCode' },
      },
    ]);
    editing.set(false);
    cb.onBlurBlock();
  };

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
          <input
            aria-label={jinshuText('@jinshu.editor.block.codeFilename')}
            value={fileDraft.value}
            placeholder={jinshuText('@jinshu.editor.block.codeFilenamePlaceholder')}
            onInput={v => fileDraft.set(v)}
            className={b.code.filename}
          />
          <text aria-hidden className={r.code.lang}>·</text>
          <input
            aria-label={jinshuText('@jinshu.editor.block.codeLanguage')}
            value={langDraft.value}
            placeholder={jinshuText('@jinshu.editor.block.codeLanguagePlaceholder')}
            onInput={v => langDraft.set(v)}
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
            onInput={v => codeDraft.set(v)}
            onKeyDown={{ Escape: commit }}
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
  start: editorIcons.alignStart,
  center: editorIcons.alignCenter,
  end: editorIcons.alignEnd,
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
  const cellText = (row: number, col: number) => inlinesToShorthand(cellRuns(row, col));

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
    const runs = shorthandToInlines(text, { existingAtoms: atomsIn(cellRuns(row, col)) });
    const rows = block.rows.map(r => (r.id === target.id ? { ...r, cells: r.cells.map((c, i) => (i === col ? runs : c)) } : r));
    cb.dispatchOps([{ op: 'replace', id: block.id, blocks: [{ ...block, rows }], noteWord: { key: row === -1 ? '@jinshu.note.tableHeader' : '@jinshu.note.tableCell' } }]);
  };

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
            <IconButton
              key={al}
              label={jinshuText('@jinshu.editor.block.align', { align: al })}
              icon={ALIGN_ICON[al]}
              size="sm"
              variant={(block.align?.[selection.index] ?? 'start') === al ? 'tonal' : 'ghost'}
              onClick={() => setColAlign(selection.index, al)}
            />
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
            onInput={v => cellDraft.set(v)}
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
              },
              ArrowUp: () => {
                if (row === -1) return;
                commitCell(row, col, cellDraft.value);
                focusCell(row - 1, col);
              },
              ArrowDown: () => {
                if (row + 1 >= nRows) return;
                commitCell(row, col, cellDraft.value);
                focusCell(row + 1, col);
              },
              Escape: () => {
                commitCell(row, col, cellDraft.value);
                activePos.set(null);
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
      <TableFrame blockId={block.id} columns={nCols} testId={cb.testScope && `${cb.testScope}-table-editor-${block.id}`}>
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

/** Recursive direct-manipulation editor for `columns` so every heading/paragraph/list inside a column is editable. */
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
  const shouldStack = env.sizeClass === 'compact' || env.textScale >= 1.35;

  const commitInlineChild = (child: JinshuBlock, text: string) => {
    if (child.type !== 'paragraph' && child.type !== 'heading') return;
    if (inlinesToShorthand(child.runs) === text) return;
    const runs = shorthandToInlines(text, { inHeading: child.type === 'heading', existingAtoms: atomsIn(child.runs) });
    cb.dispatchOps([
      {
        op: 'replace',
        id: child.id,
        blocks: [child.type === 'heading' ? { id: child.id, type: 'heading', level: child.level, runs } : { id: child.id, type: 'paragraph', runs }],
      },
    ]);
  };

  return (
    <view className={shouldStack ? b.columns.stacked : b.columns.side}>
      {block.columns.map((col, colIdx) => (
        <view key={col.id} className={b.columns.column} style={shouldStack ? undefined : { flexGrow: block.widths?.[colIdx] ?? 1, flexBasis: 0 }}>
          {col.blocks.map(child => {
            if (child.type === 'list') return <ListBlockEditor key={child.id} block={child} cb={cb} />;
            if (child.type === 'callout') return <CalloutBlockEditor key={child.id} block={child} cb={cb} />;
            if (child.type === 'quote') return <QuoteBlockEditor key={child.id} block={child} cb={cb} />;
            if (child.type === 'code') return <CodeBlockEditor key={child.id} block={child} cb={cb} />;
            if (child.type === 'table') return <TableBlockEditor key={child.id} block={child} cb={cb} />;

            if (child.type === 'paragraph' || child.type === 'heading') {
              if (activeChildId.value === child.id) {
                return (
                  <textarea
                    key={child.id}
                    aria-label={jinshuText('@jinshu.editor.block.editInColumn', { type: child.type })}
                    value={childDraft.value}
                    autoFocus
                    rows={1}
                    maxRows={18}
                    onInput={v => childDraft.set(v)}
                    onBlur={() => {
                      commitInlineChild(child, childDraft.value);
                      activeChildId.set(null);
                      cb.onBlurBlock();
                    }}
                    onKeyDown={{
                      Enter: () => {
                        commitInlineChild(child, childDraft.value);
                        activeChildId.set(null);
                        cb.onBlurBlock();
                      },
                      Escape: () => activeChildId.set(null),
                    }}
                    className={
                      child.type === 'heading' ? cx(b.columns.input, r.heading.base, r.heading.level[child.level]) : cx(b.columns.input, r.paragraph.base)
                    }
                  />
                );
              }
              return (
                <view key={child.id} className={b.columns.child}>
                  {renderFallback(child)}
                  <button
                    aria-label={jinshuText('@jinshu.editor.block.editColumnBlock', { id: child.id })}
                    onClick={() => {
                      cb.onFocusBlock(child.id);
                      activeChildId.set(child.id);
                      childDraft.set(inlinesToShorthand(child.runs));
                    }}
                    className={b.columns.childButton}
                  />
                </view>
              );
            }

            return <view key={child.id}>{renderFallback(child)}</view>;
          })}
        </view>
      ))}
    </view>
  );
}

// ---------------------------------------------------------------------------
// Inspectors
// ---------------------------------------------------------------------------

/** Inline inspector for Image blocks (alt, caption, display size). */
export function ImageBlockInspector({ block, cb }: { readonly block: ImageBlock; readonly cb: BlockEditorCallbacks }) {
  const altDraft = useLocal(`ed-img-alt-${block.id}`, block.alt ?? '');
  const capDraft = useLocal(`ed-img-cap-${block.id}`, block.caption ? inlinesToShorthand(block.caption) : '');

  const commit = () => {
    cb.dispatchOps([
      {
        op: 'update',
        id: block.id,
        attrs: { alt: altDraft.value, caption: capDraft.value.trim() ? shorthandToInlines(capDraft.value, { existingAtoms: atomsIn(block.caption) }) : null },
        noteWord: { key: '@jinshu.note.imageProps' },
      },
    ]);
  };

  return (
    <view className={b.inspector.root}>
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
      <input aria-label={jinshuText('@jinshu.editor.block.imageAlt')} value={altDraft.value} placeholder={jinshuText('@jinshu.editor.block.imageAlt')} onInput={v => altDraft.set(v)} onBlur={commit} className={b.inspector.field} />
      <input aria-label={jinshuText('@jinshu.editor.block.imageCaption')} value={capDraft.value} placeholder={jinshuText('@jinshu.editor.block.imageCaptionPlaceholder')} onInput={v => capDraft.set(v)} onBlur={commit} className={b.inspector.field} />
    </view>
  );
}

/** Inline inspector for Video, Audio and File blocks. */
export function MediaBlockInspector({ block, cb }: { readonly block: VideoBlock | AudioBlock | FileBlock; readonly cb: BlockEditorCallbacks }) {
  const primary =
    block.type === 'video' ? (block.caption ? inlinesToShorthand(block.caption) : '') : block.type === 'audio' ? block.title ?? '' : block.name;
  const secondary = block.type === 'file' ? block.description ?? '' : '';
  const primaryDraft = useLocal(`ed-media-p-${block.id}`, primary);
  const secondaryDraft = useLocal(`ed-media-s-${block.id}`, secondary);

  const commit = () => {
    if (primaryDraft.value === primary && secondaryDraft.value === secondary) return;
    const attrs: Record<string, unknown> =
      block.type === 'video'
        ? { caption: primaryDraft.value.trim() ? shorthandToInlines(primaryDraft.value, { existingAtoms: atomsIn(block.caption) }) : null }
        : block.type === 'audio'
          ? { title: primaryDraft.value.trim() || null }
          : { name: primaryDraft.value.trim() || block.name, description: secondaryDraft.value.trim() || null };
    cb.dispatchOps([{ op: 'update', id: block.id, attrs, noteWord: { key: '@jinshu.note.mediaProps' } }]);
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
