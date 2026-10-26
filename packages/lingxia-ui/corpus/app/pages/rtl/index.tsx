import type { View } from '@lingxia/ui';

export function render({ data }: View<object, { rows: { title: string; detail: string }[] }>) {
  return (
    <page className="bg-canvas">
      <column className="p-4 gap-3">
        <text role="heading" className="text-xl font-semibold text-start">הגדרות רשת</text>
        {data.rows.map(row => (
          <button key={row.title} onClick={() => {}} className="ps-4 pe-3 py-3 rounded-xl bg-surface border border-line">
            <row className="items-center gap-3">
              <column className="grow gap-0.5">
                <text className="text-sm font-medium text-start">{row.title}</text>
                <text className="text-xs text-muted text-start">{row.detail}</text>
              </column>
              <vector accessibilityHidden className="w-4 h-4 text-faint rtl:-scale-x-100" paths={[{ d: 'M9 18l6-6-6-6', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
            </row>
          </button>
        ))}
      </column>
    </page>
  );
}
