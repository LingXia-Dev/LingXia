import { PageSection, Stage } from '../shared/Layout';

const scale = [
  { name: 'Display', spec: '28/34 · Semibold', className: 'text-display font-semibold tracking-tight', sample: 'Good morning' },
  { name: 'Title', spec: '20/28 · Semibold', className: 'text-title font-semibold tracking-tight', sample: 'Delete this note?' },
  { name: 'Callout', spec: '17/24 · Medium', className: 'text-callout font-medium', sample: 'Back up your notes' },
  { name: 'Body', spec: '15/22 · Regular', className: 'text-body', sample: 'Keep a copy on another device in case this one is lost.' },
  { name: 'Footnote', spec: '13/18 · Regular', className: 'text-footnote', sample: 'Edited yesterday at 18:40' },
  { name: 'Caption', spec: '12/16 · Medium', className: 'text-caption font-medium', sample: 'Beta' },
];

const weights = [
  { name: 'Regular · 400', className: 'font-normal', use: 'Reading text' },
  { name: 'Medium · 500', className: 'font-medium', use: 'Labels and names' },
  { name: 'Semibold · 600', className: 'font-semibold', use: 'Titles' },
];

const inks = [
  { name: 'text', className: 'text-text', sample: 'What the person reads first' },
  { name: 'text-muted', className: 'text-muted', sample: 'Help, captions and second lines' },
  { name: 'text-faint', className: 'text-faint', sample: 'Placeholders and the least important hints' },
];

const fontSample = [
  '@font-face {',
  '  font-family: "Brand Sans";',
  '  src: url("../public/brand.ttf");',
  '}',
  '@theme {',
  '  --font-sans: "Brand Sans";',
  '}',
  '',
  '<page className="font-sans">…</page>',
].join('\n');

export function Content() {
  return (
    <>
      <text className="text-body text-muted">
        Six sizes, each with its own line height, in the system typeface. Body is the default; reach for the others to set a hierarchy, not for emphasis.
      </text>
      <PageSection title="Scale">
        <view className="rounded-container border border-line bg-surface divide-y divide-line">
          {scale.map(step => (
            <view key={step.name} className="gap-1 px-4 py-4">
              <text className={`text-text ${step.className}`}>{step.sample}</text>
              <text className="text-footnote text-muted">{`${step.name} · ${step.spec}`}</text>
            </view>
          ))}
        </view>
      </PageSection>
      <PageSection title="Weights">
        <Stage caption="Three weights; nothing lighter or heavier">
          {weights.map(weight => (
            <view key={weight.name} className="flex-row items-baseline justify-between gap-3">
              <text className={`text-callout text-text ${weight.className}`}>{weight.name}</text>
              <text className="text-footnote text-muted">{weight.use}</text>
            </view>
          ))}
        </Stage>
      </PageSection>
      <PageSection title="Text colour">
        <Stage caption="Three roles carry all text on a surface">
          {inks.map(ink => (
            <view key={ink.name} className="gap-0.5">
              <text className={`text-body ${ink.className}`}>{ink.sample}</text>
              <text className="text-caption text-faint">{ink.name}</text>
            </view>
          ))}
        </Stage>
      </PageSection>
      <PageSection title="Your own typeface">
        <Stage caption="Liuli names no typeface, so its components take the page's">
          <text className="text-body text-text">
            Bundle the font file, declare it with @font-face in your theme, point ‑‑font‑sans at it and set font‑sans on each page. Text inherits its family from the page, so every Liuli component follows.
          </text>
          <view className="rounded-control bg-surface-sunken p-3">
            <text className="whitespace-pre-wrap font-mono text-caption text-text">{fontSample}</text>
          </view>
        </Stage>
      </PageSection>
    </>
  );
}
