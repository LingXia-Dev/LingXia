import type { VectorPath } from '@lingxia/ui';
import '../../theme/index.css';
import { Page, Section } from '../../shared/ui';

// GPU-only renderer fixtures: one section per View capability, addressed by testId.
export function render() {
  return (
    <Page testId="view-lab" title="View lab">
      <Gradients />
      <Vectors />
      <Borders />
    </Page>
  );
}

function Gradients() {
  return (
    <Section title="Gradients" testId="lab-gradients">
      <row className="gap-3">
        <view testId="lab-gradient-radial" className="h-16 grow basis-0 rounded-xl bg-radial from-primary to-transparent" />
        <view testId="lab-gradient-conic" className="h-16 grow basis-0 rounded-xl bg-conic-90 from-primary via-error to-primary" />
        <view testId="lab-gradient-arbitrary" className="h-16 grow basis-0 rounded-xl bg-[radial-gradient(circle_at_top,#ffffff,#1677ff_70%)]" />
      </row>
    </Section>
  );
}

const star = 'M12 2l2.9 6.9 7.1.6-5.4 4.7 1.7 7-6.3-3.9-6.3 3.9 1.7-7L2 9.5l7.1-.6z';
const strokes: VectorPath[] = [
  { d: 'M3 6H21', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' },
  { d: 'M3 12H21', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeDasharray: [3, 2], strokeDashoffset: 1 },
  { d: 'M3 21L12 15L21 21', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round', strokeLinecap: 'square' },
];

function Vectors() {
  return (
    <Section title="Vectors" testId="lab-vectors">
      <row className="gap-4 items-center">
        <vector testId="lab-vector-gradient" className="w-16 h-16" paths={[{ d: star, fill: 'url(#gold)', stroke: 'url(#edge)', strokeWidth: 0.75 }]}
          gradients={{
            gold: { type: 'radial', fx: 0.35, fy: 0.3, stops: [{ offset: 0, color: '#fff3b0' }, { offset: 1, color: '#f59e0b' }] },
            edge: { type: 'linear', x2: 0, y2: 1, stops: [{ offset: 0, color: '#b45309' }, { offset: 1, color: '#78350f', opacity: 0.6 }] },
          }} />
        <vector testId="lab-vector-strokes" className="w-16 h-16 text-foreground" paths={strokes} />
        <vector testId="lab-vector-transform" className="w-16 h-16 text-primary"
          paths={[0, 45, 90, 135].map(angle => ({ d: 'M12 4h1v16h-1z', transform: `rotate(${angle} 12.5 12)` }))} />
      </row>
    </Section>
  );
}

function Borders() {
  return (
    <Section title="Borders" testId="lab-borders">
      <row className="gap-3">
        <view testId="lab-border-dashed" className="h-12 grow basis-0 rounded-xl border-2 border-dashed border-primary" />
        <view testId="lab-border-dotted" className="h-12 grow basis-0 rounded-full border-4 border-dotted border-error" />
        <view testId="lab-border-edges" className="h-12 grow basis-0 border-t-2 border-l-4 border-dashed border-foreground" />
      </row>
    </Section>
  );
}
