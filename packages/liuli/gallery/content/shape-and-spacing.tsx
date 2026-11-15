import { DensityProvider } from '@liulidesign/core/density';
import { Item, ItemList } from '@liulidesign/core/Item';
import { Select } from '@liulidesign/core/Select';
import { TextInput } from '@liulidesign/core/TextInput';
import { PageSection, Stage, StageGrid } from '../shared/Layout';

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

const roles = [{ value: 'editor', label: 'Editor' }, { value: 'viewer', label: 'Viewer' }];

/** The same short form and list, for the density comparison; ids keep the two copies apart. */
function Sample({ id }: { id: string }) {
  return (
    <view className="gap-4">
      <TextInput label="Name" value="Ana Lima" onChange={() => {}} data-testid={`${id}-name`} />
      <Select label="Role" value="editor" onChange={() => {}} options={roles} data-testid={`${id}-role`} />
      <ItemList label="Members">
        <Item label="Kai Chen" description="Editor" hasChevron onClick={() => {}} data-testid={`${id}-row`} />
        <Item label="Mira Patel" description="Viewer" hasChevron onClick={() => {}} />
      </ItemList>
    </view>
  );
}

export function Content() {
  return (
    <>
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
          <view className="gap-3">
            {heights.map(height => (
              <view key={height.name} className={`flex-row items-center justify-between rounded-control border border-line-strong px-4 ${height.bar}`}>
                <text className="text-body text-text">{height.name}</text>
                <text className="text-footnote text-muted">{height.value}</text>
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
      <PageSection title="Focus">
        <text className="text-body text-muted">
          Keyboard focus follows the shape. A control that is a row (a list item, a disclosure, a menu item, an underline tab, a checkbox, radio or switch row) fills with a rounded wash. A control that is a box (a button, a select, a text field, a radio card, a segment, a thumb) takes a ring that follows its corners.
        </text>
        <StageGrid>
          <Stage caption="A row: a rounded fill">
            <view className="-mx-2 h-control-md flex-row items-center rounded-control bg-text/7 px-2">
              <text className="text-body text-text">Notifications</text>
            </view>
          </Stage>
          <Stage caption="A box: an offset ring">
            <view className="min-h-control-md items-center justify-center rounded-control border border-line-strong ring-2 ring-focus ring-offset-2">
              <text className="text-body text-text">Save</text>
            </view>
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title="Density">
        <Stage caption="Comfortable, the default: 40-point fields and roomy rows">
          <Sample id="comfortable" />
        </Stage>
        <Stage caption="Compact, inside a DensityProvider: for dense desktop screens with a pointer, not a phone default. A control's label and value step down one size with it; row text does not change">
          <DensityProvider value="compact">
            <Sample id="compact" />
          </DensityProvider>
        </Stage>
      </PageSection>
      <PageSection title="Rhythm">
        <view className="rounded-container border border-text/12 bg-surface divide-y divide-text/8">
          {rhythm.map(entry => (
            <view key={entry.name} className="flex-row items-center justify-between gap-3 px-4 py-3">
              <text className="text-body text-text">{entry.name}</text>
              <text className="text-body text-muted">{entry.value}</text>
            </view>
          ))}
        </view>
      </PageSection>
    </>
  );
}
