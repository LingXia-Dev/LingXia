import { cx, useBox, useContext, useLocal, useView, type Child, type Local } from '@lingxia/view';
import { Button } from '../Button/Button';
import { CheckboxInput } from '../CheckboxInput/CheckboxInput';
import { checkboxInputRecipe } from '../CheckboxInput/CheckboxInput.recipe';
import { useLargeText } from '../internal/largeText';
import { DropdownMenu, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '../DropdownMenu/DropdownMenu';
import { IconButton } from '../IconButton/IconButton';
import { Skeleton, SkeletonGroup } from '../Skeleton/Skeleton';
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
  /** Draws placeholder rows under the headings, in place of `rows`, while they load. @default false */
  isLoading?: boolean;
  /**
   * The order the rows are shown in; the table sorts `rows` by it. Leave it out to let the headings own it,
   * starting from `rows` as given.
   */
  sort?: TableSort | null;
  /** Runs when a heading asks for another order, with it. */
  onSortChange?: (sort: TableSort) => void;
  /** The rows checked, by key. @default [] */
  checkedRows?: readonly string[];
  /**
   * Puts a checkbox at each row's start and one for all rows above them; runs with the keys checked, in the
   * rows' order. A row can still open its record with `onChange` or `bind`.
   */
  onCheckedRowsChange?: (keys: string[]) => void;
  className?: string;
  /** On the table; a row is `<id>-<key>`. */
  'data-testid'?: string;
};

// A text column narrower than this many characters breaks words, unless its values are shorter; the table stacks instead.
const minText = 11;
// The row's side padding and the gap between columns, in points, as the recipe sets them.
const [inset, gap] = [16, 12];
// A row's checkbox and its cell take about this much of the row from the columns: 16 inset, a 16-point box, 12 to the first column, less the 16 the row's own inset no longer takes.
const selectWidth = 28;
// Enough placeholder rows to read as a table without promising how many will come.
const loadingRows = 4;
// The bars' widths, in turn down the rows and across the columns.
const barWidths = ['long', 'half', 'most', 'short'] as const;
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
  label, columns, rows, keyField, layout = 'auto', density: ownDensity, maxHeight, isHeaderSticky = false, bind, onChange: ownChange, value: own, emptyLabel, isLoading = false, sort: ownSort, onSortChange, checkedRows = [], onCheckedRowsChange, className, 'data-testid': testId, ...attributes
}: TableProps) {
  const selectedKey = bind ? bind.value : own;
  const localSort = useLocal<TableSort | null>('sort', null);
  const sort = ownSort !== undefined ? ownSort : localSort.value;
  const setSort = (next: TableSort) => { if (ownSort === undefined) localSort.set(next); onSortChange?.(next); };
  const isSortable = columns.some(column => column.isSortable);
  const isMulti = onCheckedRowsChange !== undefined;
  const checked = new Set(checkedRows);
  // Bound, each row is a button that selects it.
  const onRowClick = bind ? (key: string) => bind.set(key) : ownChange;
  const screenDensity = useDensity();
  const density = ownDensity ?? screenDensity;
  const layer = useContext(LayerContext);
  const { env } = useView();
  // At accessibility text sizes a record's figure goes under its name, so a name never breaks inside a word.
  const isLarge = useLargeText();
  const box = useBox('table');
  const measured = useBox('sample');
  const char = measured.value ? measured.value.width / sample.length : 8;
  // A number column's longest value or heading, set in the cells' type and measured.
  const figureBoxes = columns.map((_, at) => useBox(`figure-${at}`));
  const textOf = (row: TableRow, column: TableColumn) => (column.format ? column.format(row[column.field], row) : String(row[column.field] ?? ''));
  const longest = (column: TableColumn) => rows.reduce((most, row) => Math.max(most, textOf(row, column).length), column.header.length);
  const longestText = (column: TableColumn) => rows.reduce((most, row) => (textOf(row, column).length > most.length ? textOf(row, column) : most), column.header);
  const isFigure = (column: TableColumn) => column.align === 'end' && column.cell === undefined;
  const widest = (column: TableColumn) => figureBoxes[columns.indexOf(column)]?.value?.width;
  // Until the table is measured, a phone is taken to be 328 points wide inside its page's margins.
  const isStacked = layout === 'stacked'
    || (layout === 'auto' && isCramped(columns, (box.value?.width ?? (env.sizeClass === 'compact' ? 328 : 720)) - (isMulti ? selectWidth : 0), char, longest, widest));
  // Stacked, the first field heads the record, the last number sits at the end of its line, the rest follow.
  const lead = columns[0];
  const figure = [...columns].reverse().find(column => column !== lead && column.align === 'end');
  const rest = columns.filter(column => column !== lead && column !== figure);
  // Every row is a grid of the same tracks, none sized by its content, so the columns line up.
  const tracks = { gridTemplateColumns: columns.map(track).join(' ') };
  const keyOf = (row: TableRow, index: number) => String(row[keyField ?? columns[0]?.field ?? ''] ?? index);
  const sides = isMulti ? r.inset.afterSelect : r.inset.full;
  const line = cx(r.row.base, sides, r.row.density[density]);
  const ordered = sorted(rows, columns, sort);
  const keys = ordered.map((row, index) => keyOf(row, index));
  const checkedCount = keys.filter(key => checked.has(key)).length;
  const toggle = (key: string, isOn: boolean) => onCheckedRowsChange?.(keys.filter(each => (each === key ? isOn : checked.has(each))));
  const selectAll = isMulti && (
    <view className={r.select.base}>
      <CheckboxInput label={liuliText('@liuli.selectAll')} isLabelHidden size="sm" value={keys.length > 0 && checkedCount === keys.length}
        isIndeterminate={checkedCount > 0} isDisabled={isLoading || keys.length === 0} onChange={isOn => onCheckedRowsChange?.(isOn ? keys : [])} data-testid={testId && `${testId}-select-all`} />
    </view>
  );

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
  const isSticky = isHeaderSticky && maxHeight === undefined;
  // The line under the headings: the headings' own, or, selectable, the one that puts the checkbox for all before them.
  const edge = cx(r.header.line, r.header.layer[layer], isSticky && r.header.sticky);
  const headings = (
    <view aria-hidden={!isSortable || undefined} style={tracks} className={cx(r.header.base, isMulti ? r.header.grow : edge, r.header.density[density], sides)}>
      {columns.map(heading)}
    </view>
  );
  const header = isMulti ? <view className={cx(edge, r.header.select)}>{selectAll}{headings}</view> : headings;
  // Stacked there are no headings to press: a menu picks the column, a button beside it turns the order round.
  const sortable = columns.filter(column => column.isSortable);
  const active = sortable.find(column => column.field === sort?.field);
  const sortBar = (isSortable || isMulti) && (
    <view className={cx(r.sortBar.base, isMulti ? r.inset.select : r.inset.full)}>
      {selectAll}
      {isSortable && <view role="group" aria-label={liuliText('@liuli.sort')} className={r.sortGroup.base}>
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
      </view>}
    </view>
  );

  const body = ordered.map((row, index) => {
    const key = keys[index];
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
        <view key="lead" className={isLarge ? r.lead.large : r.lead.base}>
          {lead && <view {...read(lead, isRead)} className={r.leadCell.base}>{drawn(lead, isRead, cx(r.text.base, r.text.lead))}</view>}
          {figure && <view {...read(figure, isRead)} className={r.figure.base}>{drawn(figure, isRead, cx(r.text.base, isLarge ? r.text.figureUnder : r.text.align.end))}</view>}
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
    const shape = isStacked ? { className: cx(r.block.base, sides, r.block.density[density]) } : { style: tracks, className: line };
    const rowId = testId === undefined ? undefined : `${testId}-${key}`;
    if (isMulti) {
      const isChecked = checked.has(key);
      return (
        <view key={key} role="row" className={cx(r.selectable.base, isChecked && r.selectable.checked)}>
          <view className={r.select.base}>
            <CheckboxInput label={liuliText('@liuli.selectRow', { row: lead ? textOf(row, lead) : key })} isLabelHidden size="sm" value={isChecked}
              onChange={isOn => toggle(key, isOn)} data-testid={rowId && `${rowId}-select`} />
          </view>
          {onRowClick
            ? (
              <button aria-label={columns.map(column => `${column.header}: ${textOf(row, column)}`).join(', ')} aria-selected={selectedKey === key} onClick={() => onRowClick(key)} data-testid={rowId}
                {...shape} className={cx(shape.className, r.selectable.rest, r.row.pressable, selectedKey === key ? r.row.selected : r.row.idle)}>
                {cells(false)}
              </button>
            )
            : <view data-testid={rowId} {...shape} className={cx(shape.className, r.selectable.rest)}>{cells(true)}</view>}
        </view>
      );
    }
    if (!onRowClick) return <view key={key} role="row" data-testid={rowId} {...shape}>{cells(true)}</view>;
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

  // Placeholder rows keep the table's shape: a bar per column, varied in width, the number's at the end.
  const placeholders = Array.from({ length: loadingRows }, (_, row) => (
    <view key={String(row)} className={isMulti ? r.selectable.base : r.rowgroup.base}>
      {isMulti && <view className={r.select.base}><Skeleton shape="block" className={cx(checkboxInputRecipe.box.size.sm, r.loading.box)} /></view>}
      {isStacked
        ? (
          <view className={cx(r.block.base, sides, r.block.density[density], isMulti && r.selectable.rest)}>
            <view className={isLarge ? r.lead.large : r.lead.base}>
              <view className={r.leadCell.base}><Skeleton className={r.loading.width[barWidths[row % barWidths.length]]} /></view>
              {figure && <Skeleton className={isLarge ? r.loading.figureUnder : r.loading.figure} />}
            </view>
            {rest.length > 0 && <Skeleton textSize="footnote" className={r.loading.width[barWidths[(row + 1) % barWidths.length]]} />}
          </view>
        )
        : (
          <view style={tracks} className={cx(line, isMulti && r.selectable.rest)}>
            {columns.map((column, at) => (
              <view key={column.field} className={r.cell.base}>
                <Skeleton className={column.align === 'end' ? r.loading.end : r.loading.width[barWidths[(row + at) % barWidths.length]]} />
              </view>
            ))}
          </view>
        )}
    </view>
  ));

  return (
    <view box={box} role="grid" aria-label={label} aria-busy={isLoading || undefined} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, r.root.layer[layer], className)}>
      {/* Stacked, every field is beside its own heading or needs none. */}
      {isStacked ? sortBar : header}
      {isLoading && <SkeletonGroup label={liuliText('@liuli.loadingNamed', { name: label })} className={cx(r.body.base, r.loading.group)}>{placeholders}</SkeletonGroup>}
      {!isLoading && rows.length === 0 && emptyLabel !== undefined && <text className={r.empty.base}>{emptyLabel}</text>}
      {!isLoading && rows.length > 0 && (maxHeight === undefined
        ? <view className={r.body.base}>{body}</view>
        : <scroll style={{ maxHeight }} className={r.body.base}>{body}</scroll>)}
      <view aria-hidden className={r.measure.base}>
        <text box={measured} className={r.measure.sample}>{sample}</text>
        {columns.map((column, at) => isFigure(column) && <text key={column.field} box={figureBoxes[at]} className={r.measure.sample}>{longestText(column)}</text>)}
      </view>
    </view>
  );
}
