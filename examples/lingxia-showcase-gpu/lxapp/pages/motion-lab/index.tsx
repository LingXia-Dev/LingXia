import { useLocal, useView, type View } from '@lingxia/view';
import '../../theme/index.css';
import { Button, Page, Section } from '../../shared/ui';

// Device acceptance fixtures for motion; the page opts into the gated lowering.
interface Actions {
  interrupt(): void;
}

export function render({ data, actions }: View<Actions, { dipped: boolean }>) {
  const shown = useLocal('shown', true);
  const present = useLocal('present', true);
  const order = useLocal('order', ['One', 'Two', 'Three', 'Four']);
  const pager = useLocal('pager', 0);
  const visible = shown.value && !data.dipped;
  // The bottom inset specs compare with the safe area: as a pushed or relaunched copy no bar sits below.
  const { insets, safeArea } = useView().env;
  return (
    <Page testId="motion-lab" data={{ 'data-inset-bottom': String(insets.bottom), 'data-safe-bottom': String(safeArea.bottom) }}>
      <Section title="Pager" testId="motion-pager">
        <scroll data-testid="motion-pager-scroll" bind={pager} className="w-72 overflow-x-auto snap-x snap-always rounded-xl bg-surface-sunken">
          <view className="flex-row">
            {['Morning', 'Noon', 'Evening'].map(name => (
              <view key={name} className="snap-start shrink-0 w-72 h-24 items-center justify-center">
                <text className="text-lg font-semibold">{name}</text>
              </view>
            ))}
          </view>
        </scroll>
        <view className="flex-row gap-2 items-center">
          <text data-testid="motion-pager-state" className="grow text-xs text-muted tabular-nums">{`page=${pager.value}`}</text>
          <Button testId="motion-pager-next" onClick={() => pager.set((pager.value + 1) % 3)}>Next</Button>
        </view>
      </Section>
      <Section title="Pressed scale" testId="motion-press">
        <button data-testid="motion-press-button" onClick={() => {}} className="px-4 py-3 rounded-xl bg-accent text-on-accent active:scale-95 transition-transform duration-150">
          Press and hold
        </button>
      </Section>
      <Section title="Fade" testId="motion-fade">
        <view className="flex-row items-center gap-3">
          <Button testId="motion-fade-toggle" onClick={() => shown.set(!shown.value)}>{shown.value ? 'Fade out' : 'Fade in'}</Button>
          <Button testId="motion-fade-interrupt" onClick={() => actions.interrupt()}>Interrupt</Button>
          <view data-testid="motion-fade-box" className={`w-12 h-12 rounded-lg bg-accent transition-opacity duration-200 ease-out ${visible ? 'opacity-100' : 'opacity-0'}`} />
        </view>
      </Section>
      <Section title="Enter and exit" testId="motion-presence">
        <Button testId="motion-presence-toggle" onClick={() => present.set(!present.value)}>{present.value ? 'Remove' : 'Add'}</Button>
        {present.value && (
          <view data-testid="motion-presence-card" className="h-12 rounded-lg bg-accent transition duration-300 starting:opacity-0 starting:translate-y-3 exit:opacity-0 exit:-translate-y-3" />
        )}
        <text className="text-xs text-muted">Slides in on Add and out on Remove; the gap closes at once.</text>
      </Section>
      <Section title="Reorder" testId="motion-layout">
        <view className="flex-row items-center gap-3">
          <Button testId="motion-layout-rotate" onClick={() => order.set([...order.value.slice(1), order.value[0]])}>Rotate</Button>
          <Button testId="motion-layout-reverse" onClick={() => order.set([...order.value].reverse())}>Reverse</Button>
        </view>
        {order.value.map(label => (
          <view key={label} data-testid={`motion-layout-${label}`} className="flex-row h-10 px-3 items-center rounded-lg bg-surface-sunken transition-layout duration-300">
            <text>{label}</text>
          </view>
        ))}
        <text className="text-xs text-muted">Rows slide to their new places; Reduce Motion lands them at once.</text>
      </Section>
      <Section title="Spin" testId="motion-spin">
        <vector data-testid="motion-spin-icon" className="w-10 h-10 text-accent animate-spin" paths={[{ d: 'M12 3a9 9 0 1 0 9 9', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round' }]} />
      </Section>
      <Section title="Reduced motion" testId="motion-reduce">
        <view data-testid="motion-reduce-box" className="w-10 h-10 rounded-lg bg-danger motion-safe:animate-pulse motion-reduce:opacity-60" />
        <text className="text-xs text-muted">Pulses unless the system asks for reduced motion.</text>
      </Section>
    </Page>
  );
}
