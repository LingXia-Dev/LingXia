import { cx } from '@lingxia/view';
import { fieldRecipe as r } from '../Field/Field.recipe';
import type { FieldStatus } from '../Field/Field';
import { glyphBox, glyphs } from './glyphs';

const statusGlyph = { error: glyphs.error, warning: glyphs.warning, success: glyphs.success } as const;

/**
 * A form control's validation message, and the element its control names in `aria-describedby`.
 * Always mounted: a live region announces changes, not its first appearance.
 */
export function StatusRegion({ id, status }: { id: string; status: FieldStatus | undefined }) {
  const message = status?.message;
  return (
    <view id={id} role="status" className={status && message ? r.status.shown : r.status.empty}>
      {status && message ? (
        <>
          <vector aria-hidden paths={statusGlyph[status.type]} viewBox={glyphBox} className={cx(r.statusIcon.base, r.statusIcon.type[status.type])} />
          <text className={r.statusText.base}>{message}</text>
        </>
      ) : null}
    </view>
  );
}

/** Whether a status has a message to read. */
export const hasMessage = (status: FieldStatus | undefined): boolean => Boolean(status?.message);
