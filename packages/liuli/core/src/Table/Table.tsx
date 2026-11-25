import { cx, useBox, useContext, useLocal, useView, type Child, type Local } from '@lingxia/view';
import { Button } from '../Button/Button';
import { DropdownMenu, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '../DropdownMenu/DropdownMenu';
import { IconButton } from '../IconButton/IconButton';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { useDensity } from '../density';
import { LayerContext } from '../internal/layer';
import { tableRecipe as r } from './Table.recipe';
import { dataOf } from '../internal/data';

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
export type TableSortDirection = 'ascending' | 'descending';
/** The column the rows are ordered by, and which way. */
export interface TableSort {
  field: string;
  direction: TableSortDirection;
}

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
  /** Its heading orders the rows by it, ascending, then descending on a second press. @default false */
  isSortable?: boolean;
  /**
   * Orders two rows for this column, ascending; by default numbers by value and text as the reader's
   * language sorts it, numbers inside text by value, with empty values last either way.
   */
  compare?: (a: TableRow, b: TableRow) => number;
}

export type TableBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<string | null>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The row drawn and read as selected. */
      value?: string | null;
      /** Makes each row one button; runs with its key. */
      onChange?: (key: string) => void;
    };

export type TableProps = TableBinding & {
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
  /** Shown in place of the rows when there are none: "No invoices yet". */
  emptyLabel?: string;
  /**
   * The order the rows are shown in; the table sorts `rows` by it. Leave it out to let the headings own it,
   * starting from `rows` as given.
   */
  sort?: TableSort | null;
  /** Runs when a heading asks for another order, with it. */
  onSortChange?: (sort: TableSort) => void;
  className?: string;
  /** On the table; a row is `<id>-<key>`. */
  'data-testid'?: string;
};

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

const isEmpty = (value: TableValue) => value === null || value === undefined || value === '';
const compareValues = (a: TableValue, b: TableValue) => (typeof a === 'number' && typeof b === 'number'
  ? a - b
  : String(a).localeCompare(String(b), undefined, { numeric: true, sensitivity: 'base' }));

/** `rows` in `sort`'s order, stable, with empty values last both ways; as given while nothing sorts them. */
function sorted(rows: readonly TableRow[], columns: readonly TableColumn[], sort: TableSort | null | undefined): readonly TableRow[] {
  const column = sort ? columns.find(each => each.field === sort.field && each.isSortable) : undefined;
  if (!sort || !column) return rows;
  const way = sort.direction === 'ascending' ? 1 : -1;
  const order = (a: TableRow, b: TableRow) => {
    if (column.compare) return way * column.compare(a, b);
    const [x, y] = [a[column.field], b[column.field]];
    if (isEmpty(x) || isEmpty(y)) return Number(isEmpty(x)) - Number(isEmpty(y));
    return way * compareValues(x, y);
  };
  return rows.map((row, at) => ({ row, at })).sort((a, b) => order(a.row, b.row) || a.at - b.at).map(each => each.row);
}

/** The next order a heading asks for: ascending first, then the other way. */
const nextSort = (field: string, sort: TableSort | null | undefined): TableSort => ({
  field,
  direction: sort?.field === field && sort.direction === 'ascending' ? 'descending' : 'ascending',
});

const track = (column: TableColumn) => (typeof column.width === 'number' ? `${column.width}px` : `minmax(0,${column.width ?? '1fr'})`);

/** Records in rows and fields in columns, under headings that stay in view. */
export function Table({
  label, columns, rows, keyField, layout = 'auto', density: ownDensity, maxHeight, isHeaderSticky = false, bind, onChange: ownChange, value: own, emptyLabel, sort: ownSort, onSortChange, className, 'data-testid': testId, ...attributes
}: TableProps) {
  const selectedKey = bind ? bind.value : own;
  const localSort = useLocal<TableSort | null>('sort', null);
  const sort = ownSort !== undefined ? ownSort : localSort.value;
  const setSort = (next: TableSort) => { if (ownSort === undefined) localSort.set(next); onSortChange?.(next); };
  const isSortable = columns.some(column => column.isSortable);
  // Bound, each row is a button that selects it.
  const onRowClick = bind ? (key: string) => bind.set(key) : ownChange;
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

  const directionOf = (column: TableColumn) => (sort?.field === column.field ? sort.direction : undefined);
  // Its state is in its name until the View reads a column's sort.
  const sortName = (column: TableColumn) => {
    const direction = directionOf(column);
    if (direction === 'ascending') return liuliText('@liuli.sortedAscending', { column: column.header });
    if (direction === 'descending') return liuliText('@liuli.sortedDescending', { column: column.header });
    return liuliText('@liuli.sortBy', { column: column.header });
  };
  const heading = (column: TableColumn) => {
    const align = column.align ?? 'start';
    // Each cell is read with its heading, so a heading that only names its column is not read on its own.
    if (!column.isSortable) return <text key={column.field} aria-hidden className={cx(r.heading.base, r.heading.align[align])}>{column.header}</text>;
    const direction = directionOf(column);
    return (
      <button key={column.field} aria-label={sortName(column)} onClick={() => setSort(nextSort(column.field, sort))} data-testid={testId && `${testId}-sort-${column.field}`} className={cx(r.sortHeading.base, r.sortHeading.align[align])}>
        <text className={cx(r.heading.base, direction && r.heading.sorted)}>{column.header}</text>
        <Glyph name={direction === 'descending' ? 'arrow-down' : direction === 'ascending' ? 'arrow-up' : 'sort'} className={cx(r.sortMark.base, direction ? r.sortMark.on : r.sortMark.off)} />
      </button>
    );
  };
  const header = (
    <view aria-hidden={!isSortable || undefined} style={tracks} className={cx(r.header.base, r.header.density[density], r.header.layer[layer], isHeaderSticky && maxHeight === undefined && r.header.sticky)}>
      {columns.map(heading)}
    </view>
  );
  // Stacked there are no headings to press: a menu picks the column, a button beside it turns the order round.
  const sortable = columns.filter(column => column.isSortable);
  const active = sortable.find(column => column.field === sort?.field);
  const sortBar = isSortable && (
    <view role="group" aria-label={liuliText('@liuli.sort')} className={r.sortBar.base}>
      <DropdownMenu label={liuliText('@liuli.sort')} menu={
        <DropdownMenuRadioGroup value={active?.field ?? null} onChange={field => setSort({ field, direction: sort?.direction ?? 'ascending' })}>
          {sortable.map(column => <DropdownMenuRadioItem key={column.field} value={column.field} label={column.header} />)}
        </DropdownMenuRadioGroup>
      }>
        <Button variant="ghost" size="sm" edge="start" startIcon="sort" endIcon="chevron-down" label={active?.header ?? liuliText('@liuli.sort')} data-testid={testId && `${testId}-sort`} />
      </DropdownMenu>
      {active && sort && (
        <IconButton size="sm" icon={sort.direction === 'descending' ? 'arrow-down' : 'arrow-up'}
          label={liuliText(sort.direction === 'descending' ? '@liuli.sortAscending' : '@liuli.sortDescending')}
          onClick={() => setSort({ field: active.field, direction: sort.direction === 'descending' ? 'ascending' : 'descending' })}
          data-testid={testId && `${testId}-direction`} />
      )}
    </view>
  );

  const body = sorted(rows, columns, sort).map((row, index) => {
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
    <view box={box} role="grid" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, r.root.layer[layer], className)}>
      {/* Stacked, every field is beside its own heading or needs none. */}
      {isStacked ? sortBar : header}
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
