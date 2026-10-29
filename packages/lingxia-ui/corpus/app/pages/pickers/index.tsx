// The View's own date and time pickers over a booking form. `__picker` states open one on an
// editor, as a tap would; `__pointer: "fine"` draws the desktop popover instead of the sheet.
import { useLocal } from '@lingxia/ui';

export function render() {
  const day = useLocal('2026-10-21'), at = useLocal('18:30');
  const field = 'px-3 py-2 rounded-lg border border-line-strong bg-surface text-base';
  return (
    <page className="bg-surface-sunken">
      <column className="p-4 gap-3">
        <text role="heading" headingLevel={1} className="text-xl font-semibold">Book a table</text>
        <text className="text-sm text-muted">Pick a day and a time.</text>
        <input data-testid="day" type="date" bind={day} min="2026-10-05" max="2026-12-20" aria-label="Day" className={field} />
        <input data-testid="at" type="time" bind={at} step={900} aria-label="Time" className={field} />
      </column>
    </page>
  );
}
