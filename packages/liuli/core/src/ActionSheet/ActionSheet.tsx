import { cx, useId, useLocal, useView } from '@lingxia/view';
import type { IconRef } from '../Icon/Icon';
import { FirstLine } from '../internal/firstLine';
import { Glyph, useTextGrowth } from '../internal/Glyph';
import { liuliText } from '../i18n/text';
import { actionSheetRecipe as r } from './ActionSheet.recipe';
import { dataOf } from '../internal/data';

export type ActionSheetItemVariant = 'default' | 'destructive';

// Where the sheet sits, then off screen.
const sheetPoints: readonly (number | `${number}%`)[] = [0, '100%'];

/** One action in the sheet. */
export interface ActionSheetItem {
  /** Keys the row; unique within the sheet. @default its place in the list */
  id?: string;
  /** The row's text and its accessible name. */
  label: string;
  /** An icon by name from the set, or a definition, before the label; hidden from assistive tech. */
  icon?: IconRef;
  /** Runs on choice, after the sheet has started to close. */
  onClick: () => void;
  /** `destructive` draws the row in the danger colour, for something that cannot be undone. @default 'default' */
  variant?: ActionSheetItemVariant;
  /** @default false */
  isDisabled?: boolean;
  'data-testid'?: string;
}

export interface ActionSheetProps {
  /** Whether it is shown. */
  isOpen: boolean;
  /** Called with `false` on Cancel, Escape, back, a tap outside or a drag off, and when an action is chosen. */
  onOpenChange: (isOpen: boolean) => void;
  /** The title, and the sheet's accessible name: what the actions act on ("This photo"). */
  label: string;
  /** Keeps `label` as the name without drawing it. @default false */
  isLabelHidden?: boolean;
  /** One or two sentences under the title, read as the sheet's description. */
  description?: string;
  /** The actions, in order. */
  items: readonly ActionSheetItem[];
  /** Names the cancel row. Defaults to "Cancel" in the page's language. */
  cancelLabel?: string;
  'data-testid'?: string;
}

/**
 * A short list of actions about one thing, with Cancel apart from them. On a phone it is a sheet on the bottom
 * edge; on a wide window a small panel in the middle. Escape, back, a tap outside and a drag off all cancel.
 */
export function ActionSheet({
  isOpen,
  onOpenChange,
  label,
  isLabelHidden = false,
  description,
  items,
  cancelLabel = liuliText('@liuli.cancel'),
  'data-testid': testId, ...attributes
}: ActionSheetProps) {
  const form = useView().env.sizeClass === 'compact' ? 'sheet' : 'floating';
  const hasIcons = items.some(item => item.icon !== undefined);
  const blankIcon = useTextGrowth(hasIcons ? 20 : undefined, 1.5);
  const descriptionId = `${useId()}-description`;
  // On a phone the sheet sits at its one detent, or leaves by a drag; the next open starts in place.
  const detent = useLocal('detent', 0);
  const close = () => {
    detent.set(0);
    onOpenChange(false);
  };
  const row = (key: string, text: string, onClick: () => void, variant: ActionSheetItemVariant, icon: IconRef | undefined, isDisabled: boolean, id: string | undefined) => (
    <button key={key} disabled={isDisabled} onClick={onClick} data-testid={id} className={cx(r.item.base, r.item.size[form], r.item.variant[variant])}>
      <view className={r.itemContent.base}>
        {hasIcons && (
          <FirstLine strut={r.itemLabel.type}>
            {icon === undefined ? <view aria-hidden style={blankIcon} className={r.itemIcon.base} /> : <Glyph name={icon} side={20} className={r.itemIcon.base} />}
          </FirstLine>
        )}
        <text className={cx(r.itemLabel.base, r.itemLabel.type)}>{text}</text>
      </view>
    </button>
  );
  const content = (
    <>
      {form === 'sheet' && <view aria-hidden className={r.grabber.base} />}
      {(!isLabelHidden || description !== undefined) && (
        <view className={r.header.base}>
          {!isLabelHidden && <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>}
          {description !== undefined && <text id={descriptionId} className={r.description.base}>{description}</text>}
        </view>
      )}
      <view className={r.actions.base}>
        {items.map((item, index) => row(item.id ?? String(index), item.label, () => { close(); item.onClick(); }, item.variant ?? 'default', item.icon, item.isDisabled ?? false, item['data-testid']))}
      </view>
      <view className={r.cancel.base}>
        {row('cancel', cancelLabel, close, 'default', undefined, false, testId && `${testId}-cancel`)}
      </view>
    </>
  );
  const describedBy = description !== undefined ? descriptionId : undefined;
  const className = cx(r.panel.base, r.panel[form]);
  // On a phone it follows a drag, and a drag off screen cancels it.
  return form === 'sheet'
    ? (
      <dialog open={isOpen} onClose={close} closedby="any" aria-label={label} aria-describedby={describedBy}
        drag="y" dragSnapPoints={sheetPoints} bind={detent} data-testid={testId} {...dataOf(attributes)} className={className}>
        {content}
      </dialog>
    )
    : (
      <dialog open={isOpen} onClose={close} closedby="any" aria-label={label} aria-describedby={describedBy}
        data-testid={testId} {...dataOf(attributes)} className={className}>
        {content}
      </dialog>
    );
}
