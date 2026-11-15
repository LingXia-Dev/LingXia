import { cx, type Child } from '@lingxia/view';
import { metadataListRecipe as r } from './MetadataList.recipe';

export type MetadataListLayout = 'rows' | 'stacked';

export interface MetadataItem {
  /** Identifies the pair: its test id is `<id>-<key>`. */
  key: string;
  /** What the value is: "Owner", "Created". */
  label: string;
  /** The value as text: shown, and what is read. */
  value: string;
  /** Drawn in place of the value's text: a Badge, an Avatar with a name. The text is still what is read. */
  content?: Child;
}

export interface MetadataListProps {
  /** Names the list for assistive tech: "Details". */
  label: string;
  items: readonly MetadataItem[];
  /** `rows` puts each label beside its value; `stacked` puts it above, in `columns`. @default 'rows' */
  layout?: MetadataListLayout;
  /** With `stacked`: how many pairs share a line. @default 2 */
  columns?: 1 | 2 | 3;
  className?: string;
  'data-testid'?: string;
}

/** What is known about one thing, as labels and their values. */
export function MetadataList({ label, items, layout = 'rows', columns = 2, className, 'data-testid': testId }: MetadataListProps) {
  return (
    <view role="list" aria-label={label} data-testid={testId} className={cx(r.root.layout[layout], layout === 'stacked' && r.root.columns[columns], className)}>
      {items.map(item => (
        // Read as one pair, "Owner: Ana Lima", whatever is drawn for the value.
        <view key={item.key} role="listitem" aria-label={`${item.label}: ${item.value}`} data-testid={testId && `${testId}-${item.key}`} className={r.item.layout[layout]}>
          <text aria-hidden className={cx(r.label.base, r.label.layout[layout])}>{item.label}</text>
          {item.content !== undefined
            ? <view aria-hidden className={r.content.layout[layout]}>{item.content}</view>
            : <text aria-hidden className={cx(r.value.base, r.value.layout[layout])}>{item.value}</text>}
        </view>
      ))}
    </view>
  );
}
