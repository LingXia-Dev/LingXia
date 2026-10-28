import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Page, Section } from '../../shared/ui';

// Device acceptance fixtures for motion; the page opts into the gated lowering.
interface Actions {
  interrupt(): void;
}

export function render({ data, actions }: View<Actions, { dipped: boolean }>) {
  const shown = useLocal('shown', true);
  const present = useLocal('present', true);
  const visible = shown.value && !data.dipped;
  return (
    <Page testId="motion-lab" title="Motion lab">
      <Section title="Pressed scale" testId="motion-press">
        <button testId="motion-press-button" onClick={() => {}} className="px-4 py-3 rounded-xl bg-accent text-on-accent active:scale-95 transition-transform duration-150">
          Press and hold
        </button>
      </Section>
      <Section title="Fade" testId="motion-fade">
        <row className="items-center gap-3">
          <Button testId="motion-fade-toggle" onClick={() => shown.set(!shown.value)}>{shown.value ? 'Fade out' : 'Fade in'}</Button>
          <Button testId="motion-fade-interrupt" onClick={() => actions.interrupt()}>Interrupt</Button>
          <view testId="motion-fade-box" className={`w-12 h-12 rounded-lg bg-accent transition-opacity duration-200 ease-out ${visible ? 'opacity-100' : 'opacity-0'}`} />
        </row>
      </Section>
      <Section title="Enter and exit" testId="motion-presence">
        <Button testId="motion-presence-toggle" onClick={() => present.set(!present.value)}>{present.value ? 'Remove' : 'Add'}</Button>
        {present.value && (
          <view testId="motion-presence-card" className="h-12 rounded-lg bg-accent transition duration-300 starting:opacity-0 starting:translate-y-3 exit:opacity-0 exit:-translate-y-3" />
        )}
        <text className="text-xs text-muted">Slides in on Add and out on Remove; the gap closes at once.</text>
      </Section>
      <Section title="Spin" testId="motion-spin">
        <vector testId="motion-spin-icon" className="w-10 h-10 text-accent animate-spin" paths={[{ d: 'M12 3a9 9 0 1 0 9 9', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round' }]} />
      </Section>
      <Section title="Reduced motion" testId="motion-reduce">
        <view testId="motion-reduce-box" className="w-10 h-10 rounded-lg bg-danger motion-safe:animate-pulse motion-reduce:opacity-60" />
        <text className="text-xs text-muted">Pulses unless the system asks for reduced motion.</text>
      </Section>
    </Page>
  );
}
