import { cx } from '@lingxia/view';
import { fieldRecipe as r } from '../Field/Field.recipe';
import type { FieldStatus } from '../Field/Field';
import { Glyph } from './Glyph';


/**
 * A form control's validation message, and the element its control names in `aria-describedby`.
 * Always mounted: a live region announces changes, not its first appearance.
 */
export function StatusRegion({ id, status, className }: { id: string; status: FieldStatus | undefined; className?: string }) {
  const message = status?.message;
  return (
    <view id={id} role="status" className={cx(status && message ? r.status.shown : r.status.empty, className)}>
      {status && message ? (
        <>
          <view aria-hidden className={r.statusLine.base}>
            <text className={r.statusGhost.base}>{'\u200B'}</text>
            <Glyph name={status.type} className={cx(r.statusIcon.base, r.statusIcon.type[status.type])} />
          </view>
          <text className={r.statusText.base}>{message}</text>
        </>
      ) : null}
    </view>
  );
}

/** Whether a status has a message to read. */
export const hasMessage = (status: FieldStatus | undefined): boolean => Boolean(status?.message);
