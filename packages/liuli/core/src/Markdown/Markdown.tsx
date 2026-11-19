import { cx, type Child } from '@lingxia/view';
import { Table, type TableColumn } from '../Table/Table';
import { markdownRecipe as r } from './Markdown.recipe';
import { parseMarkdown, plainText, type MarkdownBlock, type MarkdownRun } from './parse';

export type MarkdownSize = 'sm' | 'md';

export interface MarkdownProps {
  /** The Markdown text: headings, paragraphs, emphasis, links, lists with tasks, quotes, code, rules and tables. */
  source: string;
  /** Runs with a pressed link's address; the app opens it. Without it links are drawn as plain text. */
  onLinkClick?: (href: string) => void;
  /** The heading level a `#` gets, so the text fits under the page's own headings. @default 2 */
  topHeadingLevel?: 1 | 2 | 3 | 4 | 5 | 6;
  /** What a screen reader hears for a task's mark: `{ done: 'Done', open: 'Not done' }`. Without it the marks are not read. */
  taskLabels?: { done: string; open: string };
  /** Lets people select and copy the text. @default true */
  isSelectable?: boolean;
  /** `sm` for a comment or a card. @default 'md' */
  size?: MarkdownSize;
  className?: string;
  'data-testid'?: string;
}

const tick = [{ d: 'M3.5 8.5l3 3 6-7', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' }] as const;

/** Markdown drawn as Liuli text: for a message, a note, a help page. It shows text; it does not edit it. */
export function Markdown({
  source, onLinkClick, topHeadingLevel = 2, taskLabels, isSelectable = true, size = 'md', className, 'data-testid': testId,
}: MarkdownProps) {
  const marks = (run: MarkdownRun) => cx(run.isStrong && r.run.strong, run.isEmphasis && r.run.emphasis, run.isStruck && r.run.struck, run.isCode && r.run.code);
  // Runs of one link are gathered into it: a link is one target with one name, and links do not nest.
  const inline = (runs: readonly MarkdownRun[]): Child[] => {
    const out: Child[] = [];
    for (let at = 0; at < runs.length;) {
      const href = runs[at].href;
      let end = at + 1;
      while (href !== undefined && end < runs.length && runs[end].href === href) end += 1;
      const spans = runs.slice(at, end).map((run, index) => {
        const classes = marks(run);
        return classes === '' ? run.text : <span key={String(at + index)} className={classes}>{run.text}</span>;
      });
      if (href !== undefined && onLinkClick) out.push(<link key={`link-${at}`} onClick={() => onLinkClick(href)} className={r.link.base}>{spans}</link>);
      else out.push(...spans);
      at = end;
    }
    return out;
  };

  const block = (each: MarkdownBlock, key: string): Child => {
    if (each.type === 'heading') {
      const level = Math.min(6, topHeadingLevel + each.level - 1) as 1 | 2 | 3 | 4 | 5 | 6;
      return <text key={key} role="heading" headingLevel={level} className={cx(r.heading.base, r.heading.level[Math.min(4, each.level) as 1 | 2 | 3 | 4])}>{inline(each.runs)}</text>;
    }
    if (each.type === 'paragraph') return <text key={key} className={cx(r.paragraph.base, r.paragraph.size[size])}>{inline(each.runs)}</text>;
    if (each.type === 'rule') return <view key={key} aria-hidden className={r.rule.base} />;
    if (each.type === 'quote') return <view key={key} className={r.quote.base}>{each.blocks.map((inner, index) => block(inner, String(index)))}</view>;
    if (each.type === 'code') {
      return (
        <scroll key={key} className={r.code.base}>
          <text className={r.code.text}>{each.text}</text>
        </scroll>
      );
    }
    if (each.type === 'list') {
      return (
        <view key={key} role="list" className={r.list.base}>
          {each.items.map((item, index) => (
            <view key={String(index)} role="listitem" className={r.item.base}>
              {item.isDone === undefined
                ? <text aria-hidden className={cx(r.marker.base, r.paragraph.size[size])}>{each.isOrdered ? `${each.start + index}.` : '•'}</text>
                : (
                  <view role={taskLabels ? 'img' : undefined} aria-label={taskLabels ? (item.isDone ? taskLabels.done : taskLabels.open) : undefined} aria-hidden={taskLabels ? undefined : true} className={cx(r.taskSlot.base, r.taskSlot.size[size])}>
                    <text aria-hidden className={cx(r.taskGhost.base, r.paragraph.size[size])}>{'\u200B'}</text>
                    <view className={cx(r.task.base, r.task.size[size], item.isDone ? r.task.done : r.task.open)}>
                      {item.isDone && <vector aria-hidden paths={tick} viewBox={[0, 0, 16, 16]} className={r.task.tick[size]} />}
                    </view>
                  </view>
                )}
              <view className={r.itemBody.base}>{item.blocks.map((inner, at) => block(inner, String(at)))}</view>
            </view>
          ))}
        </view>
      );
    }
    // A table: its cells keep their marks and links, and are read as their plain text.
    const columns: TableColumn[] = each.header.map((cell, column) => ({
      field: `c${column}`,
      header: plainText(cell),
      align: each.align[column] === 'end' ? 'end' : 'start',
      cell: row => <text className={cx(r.paragraph.base, r.cell.base)}>{inline(each.rows[Number(row.row)]?.[column] ?? [])}</text>,
    }));
    const rows = each.rows.map((row, index) => ({ row: index, ...Object.fromEntries(each.header.map((_, column) => [`c${column}`, plainText(row[column] ?? [])])) }));
    return <Table key={key} label={columns.map(column => column.header).join(', ')} columns={columns} rows={rows} keyField="row" density="compact" />;
  };

  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.size[size], isSelectable && r.root.selectable, className)}>
      {parseMarkdown(source).map((each, index) => block(each, String(index)))}
    </view>
  );
}
