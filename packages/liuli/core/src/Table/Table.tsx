import { cx, useBox, useContext, useView, type Child } from '@lingxia/view';
import { useDensity } from '../density';
import { LayerContext } from '../internal/layer';
import { tableRecipe as r } from './Table.recipe';

export type TableValue = string | number | null | undefined;
/** One record: its fields by name. */
export type TableRow = Readonly<Record<string, TableValue>>;
export type TableAlign = 'start' | 'end';
/** `compact` is for long tables read with a mouse: rows 36 px tall. */
export type TableDensity = 'comfortable' | 'compact';
/**
 * `columns` keeps every field in its column; `stacked` gives each record a block: its first field
 * and its number on one line, the other fields under them. `auto` stacks when a text column
 * would be too narrow to read.
 */
export type TableLayout = 'auto' | 'columns' | 'stacked';

export interface TableColumn {
  /** The row field this column shows. */
  field: string;
  /** The column's heading, and what each of its cells is read with: "Amount". */
  header: string;
  /** `end` for numbers, so their digits line up. @default 'start' */
  align?: TableAlign;
  /** The column's width in points, or its share of what is left: `'2fr'`. @default '1fr' */
  width?: number | `${number}fr`;
  /** Turns the field's value into the text shown and read. By default the value as written. */
  format?: (value: TableValue, row: TableRow) => string;
  /** Draws the cell in place of its text: a Badge, an Avatar. The text is still what is read. */
  cell?: (row: TableRow) => Child;
}

export interface TableProps {
  /** The table's accessible name: "Invoices". */
  label: string;
  columns: readonly TableColumn[];
  rows: readonly TableRow[];
  /** The field that identifies a row across updates, in `value` and in `onChange`. By default the first column's. */
  keyField?: string;
  /** @default 'auto' */
  layout?: TableLayout;
  /** @default 'comfortable' */
  density?: TableDensity;
  /** Caps the rows' height in points: the rows scroll under a heading that stays. */
  maxHeight?: number;
  /**
   * Holds the heading at the top of the scroll the table sits in while its rows pass under it.
   * The table must be inside a `<scroll>`; without `maxHeight`. @default false
   */
  isHeaderSticky?: boolean;
  /** Makes each row one button; runs with its key. */
  onChange?: (key: string) => void;
  /** The row drawn and read as selected. */
  value?: string | null;
  /** Shown in place of the rows when there are none: "No invoices yet". */
  emptyLabel?: string;
  className?: string;
  /** On the table; a row is `<id>-<key>`. */
  'data-testid'?: string;
}

// A text column narrower than this many characters breaks words, unless its values are shorter; the table stacks instead.
const minText = 11;
// The row's side padding and the gap between columns, in points, as the recipe sets them.
const [inset, gap] = [16, 12];
// Measured for a character's width at the reader's text size; taken as 8 points until it is.
const sample = 'abcdefghijklmnopqrstuvwxyz';
const share = (column: TableColumn) => (typeof column.width === 'number' ? 0 : parseFloat(column.width ?? '1fr'));

/**
 * Whether a column of a table `width` points wide is too narrow: a text column under 11 characters
 * of `char` points or under its longest value, or a number column, which never wraps, under its
 * widest value as measured.
 */
function isCramped(columns: readonly TableColumn[], width: number, char: number, longest: (column: TableColumn) => number, widest: (column: TableColumn) => number | undefined): boolean {
  const fixed = columns.reduce((sum, column) => sum + (typeof column.width === 'number' ? column.width : 0), 0);
  const unit = (width - inset * 2 - gap * (columns.length - 1) - fixed) / Math.max(1e-6, columns.reduce((sum, column) => sum + share(column), 0));
  return columns.some(column => {
    const room = typeof column.width === 'number' ? column.width : unit * share(column);
    if ((column.align ?? 'start') === 'end') return room < (widest(column) ?? 0);
    return share(column) > 0 && room < Math.min(minText, longest(column)) * char;
  });
}

const track = (column: TableColumn) => (typeof column.width === 'number' ? `${column.width}px` : `minmax(0,${column.width ?? '1fr'})`);

/** Records in rows and fields in columns, under headings that stay in view. */
export function Table({
  label, columns, rows, keyField, layout = 'auto', density: ownDensity, maxHeight, isHeaderSticky = false, onChange: onRowClick, value: selectedKey, emptyLabel, className, 'data-testid': testId,
}: TableProps) {
  const screenDensity = useDensity();
  const density = ownDensity ?? screenDensity;
  const layer = useContext(LayerContext);
  const { env } = useView();
  const box = useBox('table');
  const measured = useBox('sample');
  const char = measured.value ? measured.value.width / sample.length : 8;
  // A number column's longest value or heading, set in the cells' type and measured.
  const figureBoxes = columns.map(column => useBox(`figure-${column.field}`));
  const textOf = (row: TableRow, column: TableColumn) => (column.format ? column.format(row[column.field], row) : String(row[column.field] ?? ''));
  const longest = (column: TableColumn) => rows.reduce((most, row) => Math.max(most, textOf(row, column).length), column.header.length);
  const longestText = (column: TableColumn) => rows.reduce((most, row) => (textOf(row, column).length > most.length ? textOf(row, column) : most), column.header);
  const isFigure = (column: TableColumn) => column.align === 'end' && column.cell === undefined;
  const widest = (column: TableColumn) => figureBoxes[columns.indexOf(column)]?.value?.width;
  // Until the table is measured, a phone is taken to be 328 points wide inside its page's margins.
  const isStacked = layout === 'stacked'
    || (layout === 'auto' && isCramped(columns, box.value?.width ?? (env.sizeClass === 'compact' ? 328 : 720), char, longest, widest));
  // Stacked, the first field heads the record, the last number sits at the end of its line, the rest follow.
  const lead = columns[0];
  const figure = [...columns].reverse().find(column => column !== lead && column.align === 'end');
  const rest = columns.filter(column => column !== lead && column !== figure);
  // Every row is a grid of the same tracks, none sized by its content, so the columns line up.
  const tracks = { gridTemplateColumns: columns.map(track).join(' ') };
  const keyOf = (row: TableRow, index: number) => String(row[keyField ?? columns[0]?.field ?? ''] ?? index);
  const line = cx(r.row.base, r.row.density[density]);

  const header = (
    // Each cell is read with its heading, so the headings are not read on their own.
    <view aria-hidden style={tracks} className={cx(r.header.base, r.header.density[density], r.header.layer[layer], isHeaderSticky && maxHeight === undefined && r.header.sticky)}>
      {columns.map(column => <text key={column.field} className={cx(r.heading.base, r.heading.align[column.align ?? 'start'])}>{column.header}</text>)}
    </view>
  );

  const body = rows.map((row, index) => {
    const key = keyOf(row, index);
    const isSelected = selectedKey === key;
    const drawn = (column: TableColumn, isRead: boolean, className: string) => (column.cell
      ? <view aria-hidden={isRead || undefined} className={cx(r.custom.base, r.custom.align[column.align ?? 'start'])}>{column.cell(row)}</view>
      : <text aria-hidden={isRead || undefined} className={className}>{textOf(row, column)}</text>);
    const read = (column: TableColumn, isRead: boolean) => (isRead ? { role: 'gridcell' as const, 'aria-label': `${column.header}: ${textOf(row, column)}` } : {});
    const cells = (isRead: boolean) => (!isStacked
      ? columns.map(column => (
        <view key={column.field} {...read(column, isRead)} className={r.cell.base}>
          {drawn(column, isRead, cx(r.text.base, r.text.align[column.align ?? 'start']))}
        </view>
      ))
      : [
        <view key="lead" className={r.lead.base}>
          {lead && <view {...read(lead, isRead)} className={r.leadCell.base}>{drawn(lead, isRead, cx(r.text.base, r.text.lead))}</view>}
          {figure && <view {...read(figure, isRead)} className={r.figure.base}>{drawn(figure, isRead, cx(r.text.base, r.text.align.end))}</view>}
        </view>,
        rest.length > 0 && (
          <view key="rest" className={r.pairs.base}>
            {rest.map(column => (
              <view key={column.field} {...read(column, isRead)} className={r.pair.base}>
                <text aria-hidden={isRead || undefined} className={r.pairHeading.base}>{column.header}</text>
                {drawn(column, isRead, r.pairValue.base)}
              </view>
            ))}
          </view>
        ),
      ]);
    const shape = isStacked ? { className: cx(r.block.base, r.block.density[density]) } : { style: tracks, className: line };
    if (!onRowClick) return <view key={key} role="row" data-testid={testId === undefined ? undefined : `${testId}-${key}`} {...shape}>{cells(true)}</view>;
    return (
      <view key={key} role="row" className={r.rowgroup.base}>
        <button
          aria-label={columns.map(column => `${column.header}: ${textOf(row, column)}`).join(', ')}
          aria-selected={isSelected}
          onClick={() => onRowClick(key)}
          data-testid={testId === undefined ? undefined : `${testId}-${key}`}
          {...shape}
          className={cx(shape.className, r.row.pressable, isSelected ? r.row.selected : r.row.idle)}
        >
          {cells(false)}
        </button>
      </view>
    );
  });

  return (
    <view box={box} role="grid" aria-label={label} data-testid={testId} className={cx(r.root.base, r.root.layer[layer], className)}>
      {/* Stacked, every field is beside its own heading or needs none. */}
      {!isStacked && header}
      {rows.length === 0 && emptyLabel !== undefined && <text className={r.empty.base}>{emptyLabel}</text>}
      {rows.length > 0 && (maxHeight === undefined
        ? <view className={r.body.base}>{body}</view>
        : <scroll style={{ maxHeight }} className={r.body.base}>{body}</scroll>)}
      <view aria-hidden className={r.measure.base}>
        <text box={measured} className={r.measure.sample}>{sample}</text>
        {columns.map((column, at) => isFigure(column) && <text key={column.field} box={figureBoxes[at]} className={r.measure.sample}>{longestText(column)}</text>)}
      </view>
    </view>
  );
}
