import { PageSection } from '../shared/Layout';

// Colours are theme roles, so each sample follows the appearance; no value is written here.

interface Swatch {
  role: string;
  use: string;
  /** The sample's classes: the role as a fill, or as text or a border on the surface. */
  sample: string;
  /** Text drawn on the sample, in the role that belongs on it. */
  ink?: string;
  /** Drawn as a veil over light and dark squares, top half bare for comparison. */
  isVeil?: boolean;
}

const groups: { title: string; swatches: Swatch[] }[] = [
  {
    title: 'Surfaces',
    swatches: [
      { role: 'canvas', use: 'The page behind everything', sample: 'bg-canvas', ink: 'text-text' },
      { role: 'surface', use: 'Cards, stages and inputs', sample: 'bg-surface', ink: 'text-text' },
      { role: 'surface-raised', use: 'Raised cards and segment thumbs', sample: 'bg-surface-raised', ink: 'text-text' },
      { role: 'surface-overlay', use: 'Menus, popovers, dialogs and sheets', sample: 'bg-surface-overlay', ink: 'text-text' },
      { role: 'surface-sunken', use: 'Wells inside a surface', sample: 'bg-surface-sunken', ink: 'text-text' },
      { role: 'scrim', use: 'Dims the page behind a modal dialog', sample: 'bg-scrim', isVeil: true },
    ],
  },
  {
    title: 'Text',
    swatches: [
      { role: 'text', use: 'Body text and titles', sample: 'bg-surface', ink: 'text-text' },
      { role: 'text-muted', use: 'Secondary text and captions', sample: 'bg-surface', ink: 'text-muted' },
      { role: 'text-faint', use: 'Placeholders and hints', sample: 'bg-surface', ink: 'text-faint' },
      { role: 'selection', use: 'Selected text', sample: 'bg-selection', ink: 'text-text' },
    ],
  },
  {
    title: 'Lines',
    swatches: [
      { role: 'line', use: 'Dividers between rows', sample: 'bg-surface border-2 border-line' },
      { role: 'line-strong', use: 'Input borders, 3:1 on a surface', sample: 'bg-surface border-2 border-line-strong' },
      { role: 'focus', use: 'The ring around a focused control', sample: 'bg-surface border-2 border-focus' },
    ],
  },
  {
    title: 'Accent',
    swatches: [
      { role: 'accent', use: 'The primary action, with on-accent', sample: 'bg-accent', ink: 'text-on-accent' },
      { role: 'accent-soft', use: 'Quiet highlights, with on-accent-soft', sample: 'bg-accent-soft', ink: 'text-on-accent-soft' },
      { role: 'knob', use: 'Switch and slider thumbs, white in both appearances', sample: 'bg-accent/80', ink: 'text-knob' },
    ],
  },
  {
    title: 'Status',
    swatches: [
      { role: 'danger', use: 'Destructive actions and errors', sample: 'bg-danger', ink: 'text-on-danger' },
      { role: 'danger-soft', use: 'Behind an error label', sample: 'bg-danger-soft', ink: 'text-text' },
      { role: 'warning', use: 'Something needs a look', sample: 'bg-warning', ink: 'text-on-warning' },
      { role: 'warning-soft', use: 'Behind a warning label', sample: 'bg-warning-soft', ink: 'text-text' },
      { role: 'success', use: 'Done, or healthy', sample: 'bg-success', ink: 'text-on-success' },
      { role: 'success-soft', use: 'Behind a success label', sample: 'bg-success-soft', ink: 'text-text' },
    ],
  },
];

const series = ['bg-chart-1', 'bg-chart-2', 'bg-chart-3', 'bg-chart-4', 'bg-chart-5'];

/** A checker of surface and text squares; the scrim covers its lower half, so it reads as a veil. */
function Veil({ sample }: { sample: string }) {
  return (
    <view className="relative size-11 overflow-hidden rounded-control border border-text/12">
      <view className="grow flex-row">
        <view className="grow basis-0 bg-surface" />
        <view className="grow basis-0 bg-text" />
      </view>
      <view className="grow flex-row">
        <view className="grow basis-0 bg-text" />
        <view className="grow basis-0 bg-surface" />
      </view>
      <view className={`absolute inset-x-0 bottom-0 h-1/2 ${sample}`} />
    </view>
  );
}

function SwatchRow({ swatch }: { swatch: Swatch }) {
  return (
    <view className="flex-row items-center gap-3 px-4 py-3">
      {swatch.isVeil ? (
        <Veil sample={swatch.sample} />
      ) : (
        <view className={`size-11 items-center justify-center rounded-control border border-text/12 ${swatch.sample}`}>
          {swatch.ink !== undefined && <text className={`text-body font-semibold ${swatch.ink}`}>Aa</text>}
        </view>
      )}
      <view className="grow shrink min-w-0 gap-0.5">
        <text className="text-body font-medium text-text">{swatch.role}</text>
        <text className="text-footnote text-muted">{swatch.use}</text>
      </view>
    </view>
  );
}

export function Content() {
  return (
    <>
      <text className="text-body text-muted">
        Components never name a colour, only a role. The theme gives each role a light and a dark value, so this page changes with the appearance.
      </text>
      {groups.map(group => (
        <PageSection key={group.title} title={group.title}>
          <view className="rounded-container border border-text/12 bg-surface divide-y divide-text/8">
            {group.swatches.map(swatch => <SwatchRow key={swatch.role} swatch={swatch} />)}
          </view>
        </PageSection>
      ))}
      <PageSection title="Data">
        <view className="gap-2">
          <view className="flex-row gap-1.5">
            {series.map(fill => <view key={fill} className={`grow basis-0 h-11 rounded-control ${fill}`} />)}
          </view>
          <text className="text-footnote text-muted">chart-1 to chart-5: chart series, in this order</text>
        </view>
      </PageSection>
    </>
  );
}
