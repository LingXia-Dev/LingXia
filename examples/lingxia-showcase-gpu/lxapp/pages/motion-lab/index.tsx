// @ui-experimental motion
import { useLocal } from '@lingxia/ui';
import '../../theme/index.css';
import { Button, Page, Section } from '../../shared/ui';

// Device acceptance fixtures for motion; the page opts into the gated lowering.
export function render() {
  const shown = useLocal('shown', true);
  return (
    <Page testId="motion-lab" title="Motion lab">
      <Section title="Pressed scale" testId="motion-press">
        <button testId="motion-press-button" onClick={() => {}} className="px-4 py-3 rounded-xl bg-primary text-primary-foreground active:scale-95 transition-transform duration-150">
          Press and hold
        </button>
      </Section>
      <Section title="Fade" testId="motion-fade">
        <row className="items-center gap-3">
          <Button testId="motion-fade-toggle" onClick={() => shown.set(!shown.value)}>{shown.value ? 'Fade out' : 'Fade in'}</Button>
          <view testId="motion-fade-box" className={`w-12 h-12 rounded-lg bg-primary transition-opacity duration-200 ease-out ${shown.value ? 'opacity-100' : 'opacity-0'}`} />
        </row>
      </Section>
      <Section title="Spin" testId="motion-spin">
        <vector testId="motion-spin-icon" className="w-10 h-10 text-primary animate-spin" paths={[{ d: 'M12 3a9 9 0 1 0 9 9', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round' }]} />
      </Section>
      <Section title="Reduced motion" testId="motion-reduce">
        <view testId="motion-reduce-box" className="w-10 h-10 rounded-lg bg-error motion-safe:animate-pulse motion-reduce:opacity-60" />
        <text className="text-xs text-muted-foreground">Pulses unless the system asks for reduced motion.</text>
      </Section>
    </Page>
  );
}
