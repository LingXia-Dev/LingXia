import { GalleryPage, PageSection, Stage, StageGrid } from '../../shared/Layout';

// Every number on this page is a token from tokens.css or Tailwind's 4-point step.

const corners = [
  { name: 'Checkbox', value: '6', shape: 'size-10 rounded-checkbox' },
  { name: 'Control', value: '10, a segment inside 8', shape: 'h-10 rounded-control p-0.5', inner: 'grow w-1/2 rounded-segment border border-accent/40 bg-surface' },
  { name: 'Container', value: '14', shape: 'h-16 rounded-container' },
  { name: 'Dialog', value: '20', shape: 'h-20 rounded-dialog' },
];

const heights = [
  { name: 'Small', value: '32', bar: 'h-control-sm' },
  { name: 'Medium', value: '40', bar: 'h-control-md' },
  { name: 'Large', value: '48', bar: 'h-control-lg' },
];

const steps = [
  { step: '1', px: '4', bar: 'w-1' },
  { step: '1.5', px: '6', bar: 'w-1.5' },
  { step: '2', px: '8', bar: 'w-2' },
  { step: '3', px: '12', bar: 'w-3' },
  { step: '4', px: '16', bar: 'w-4' },
  { step: '5', px: '20', bar: 'w-5' },
  { step: '6', px: '24', bar: 'w-6' },
  { step: '8', px: '32', bar: 'w-8' },
  { step: '10', px: '40', bar: 'w-10' },
];

const rhythm = [
  { name: 'Page gutter', value: '16' },
  { name: 'Between sections', value: '32' },
  { name: 'Inside a card or stage', value: '20' },
  { name: 'Between controls in a group', value: '12' },
  { name: 'Label to its control', value: '8' },
  { name: 'Control to its help text', value: '6' },
];

export function render() {
  return (
    <GalleryPage>
      <text className="text-body text-muted">
        Corners are named for what the shape is, not how big it is. Heights and gaps come from a 4-point step.
      </text>
      <PageSection title="Corners">
        <StageGrid>
          {corners.map(corner => (
            <Stage key={corner.name} caption={`${corner.name} · ${corner.value}`}>
              <view className={`border border-accent/40 bg-accent-soft ${corner.shape}`}>
                {'inner' in corner && <view className={corner.inner} />}
              </view>
            </Stage>
          ))}
        </StageGrid>
      </PageSection>
      <PageSection title="Control heights">
        <Stage caption="Buttons, inputs and menu triggers share these three; prefer large on touch layouts">
          <view className="flex-row items-end gap-3">
            {heights.map(height => (
              <view key={height.name} className={`grow basis-0 items-center justify-center rounded-control border border-line-strong ${height.bar}`}>
                <text className="text-footnote text-text">{`${height.name} · ${height.value}`}</text>
              </view>
            ))}
          </view>
        </Stage>
      </PageSection>
      <PageSection title="Spacing steps">
        <Stage caption="Class step, then points">
          {steps.map(step => (
            <view key={step.step} className="flex-row items-center gap-3">
              <text className="w-8 text-footnote text-muted">{step.step}</text>
              <view className={`h-3 rounded-sm bg-accent ${step.bar}`} />
              <text className="text-footnote text-text">{step.px}</text>
            </view>
          ))}
        </Stage>
      </PageSection>
      <PageSection title="Rhythm">
        <view className="rounded-container border border-text/10 bg-surface divide-y divide-text/8">
          {rhythm.map(entry => (
            <view key={entry.name} className="flex-row items-center justify-between gap-3 px-4 py-3">
              <text className="text-body text-text">{entry.name}</text>
              <text className="text-body text-muted">{entry.value}</text>
            </view>
          ))}
        </view>
      </PageSection>
    </GalleryPage>
  );
}
