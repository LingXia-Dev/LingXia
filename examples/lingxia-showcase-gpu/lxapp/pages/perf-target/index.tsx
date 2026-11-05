import type { View } from '@lingxia/view';
import '../../theme/index.css';
import { Page } from '../../shared/ui';

// The page perf-lab's transition scene pushes and pops; its rows set the first frame's work.
export function render({ data }: View<{}, { rows: number }>) {
  return (
    <Page testId="perf-target">
      {Array.from({ length: data.rows }, (_, i) => (
        <row key={String(i)} className="items-center gap-3 p-3 rounded-xl bg-surface border border-line">
          <image src={`public/perf/img-${String(i % 16).padStart(2, '0')}.png`} alt="" className="w-10 h-10 rounded-lg" />
          <column className="grow basis-0 min-w-0 gap-0.5">
            <text className="text-sm font-medium truncate">Row {i + 1}</text>
            <text className="text-xs text-muted truncate">Pushed and popped by the transition scene</text>
          </column>
        </row>
      ))}
    </Page>
  );
}
