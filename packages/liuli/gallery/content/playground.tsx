import { useLocal, useView } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { ItemList } from '@liulidesign/core/Item';
import { CodeBlock } from '@liulidesign/core/Code';
import { NumberInput } from '@liulidesign/core/NumberInput';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { Select } from '@liulidesign/core/Select';
import { Switch } from '@liulidesign/core/Switch';
import { TextInput } from '@liulidesign/core/TextInput';
import { PageSection, Stage } from '../shared/Layout';
import { playgroundEntries, PlaygroundSubject, type PlaygroundControl, type PlaygroundProps } from '../shared/playground.generated';

type Value = string | number | boolean | undefined;

// Segments hold a few short words; anything longer or optional goes in a select, which can be empty.
const fitsSegments = (control: PlaygroundControl, value: Value) =>
  value !== undefined && control.options!.length <= 4 && control.options!.join('').length <= 24;

function Control({ control, value, onChange }: { control: PlaygroundControl; value: Value; onChange: (value: Value) => void }) {
  const testId = `prop-${control.name}`;
  if (control.kind === 'boolean') return <Switch label={control.name} value={value === true} onChange={onChange} variant="row" data-testid={testId} />;
  if (control.kind === 'number')
    return (
      <NumberInput
        label={control.name}
        value={typeof value === 'number' ? value : null}
        onChange={next => onChange(next ?? undefined)}
        min={0}
        decrementLabel={`Lower ${control.name}`}
        incrementLabel={`Raise ${control.name}`}
        size="sm"
        data-testid={testId}
      />
    );
  if (control.kind === 'text')
    return (
      <TextInput
        label={control.name}
        value={typeof value === 'string' ? value : ''}
        onChange={next => onChange(next === '' && !control.required ? undefined : next)}
        placeholder={control.required ? undefined : 'Not set'}
        autoCapitalize="none"
        size="sm"
        data-testid={testId}
      />
    );
  const options = control.options!.map(option => ({ value: option, label: option }));
  if (fitsSegments(control, value))
    return (
      // Segments carry no visible label of their own; this one matches the fields around it.
      <view className="gap-1.5">
        <text aria-hidden className="pb-0.5 text-caption font-medium text-text">{control.name}</text>
        <SegmentedControl label={control.name} options={options} value={String(value)} onChange={onChange} size="sm" data-testid={testId} />
      </view>
    );
  return (
    <Select
      label={control.name}
      value={typeof value === 'string' ? value : ''}
      onChange={next => onChange(next === '' ? undefined : next)}
      placeholder="Not set"
      options={options}
      size="sm"
      data-testid={testId}
    />
  );
}

/** The JSX that draws what the stage shows: one line when short, a prop per line when not. */
function codeFor(name: string, props: PlaygroundProps, order: string[]) {
  const written = order
    .filter(prop => props[prop] !== undefined && props[prop] !== false)
    .map(prop => {
      const value = props[prop];
      return value === true ? prop : typeof value === 'number' ? `${prop}={${value}}` : `${prop}=${JSON.stringify(value)}`;
    });
  const line = `<${name} ${written.join(' ')} />`;
  return line.length <= 44 ? line : `<${name}\n${written.map(each => `  ${each}`).join('\n')}\n/>`;
}

const kindOrder = { text: 0, number: 1, choice: 2, boolean: 3 };


export function Content() {
  const chosen = useLocal('component', 'Button');
  const edits = useLocal<Record<string, PlaygroundProps>>('edits', {});
  const entry = playgroundEntries.find(each => each.name === chosen.value) ?? playgroundEntries[0];
  const props = edits.value[entry.name] ?? entry.defaults;
  const fields = entry.controls.filter(control => control.kind !== 'boolean').sort((a, b) => kindOrder[a.kind] - kindOrder[b.kind]);
  const flags = entry.controls.filter(control => control.kind === 'boolean');
  const set = (name: string, value: Value) => {
    const next = { ...props };
    if (value === undefined) delete next[name];
    else next[name] = value;
    edits.set({ ...edits.value, [entry.name]: next });
  };
  const reset = () => {
    const { [entry.name]: _, ...rest } = edits.value;
    edits.set(rest);
  };
  // On a phone the component stays put while its props scroll under it; beside the gallery's sidebar the two sit side by side.
  const isWide = useView().env.sizeClass === 'regular';
  const row = (control: PlaygroundControl) => (
    <Control key={`${entry.name}-${control.name}`} control={control} value={props[control.name]} onChange={value => set(control.name, value)} />
  );
  const picker = (
    <Select
      label="Component"
      isLabelHidden={!isWide}
      bind={chosen}
      options={playgroundEntries.map(each => ({ value: each.name, label: each.name }))}
      data-testid="playground-component"
    />
  );
  const stage = (
    <Stage>
      <view className="min-h-20 justify-center" data-testid="playground-stage">
        <PlaygroundSubject name={entry.name} props={props} />
      </view>
    </Stage>
  );
  const code = <CodeBlock code={codeFor(entry.name, props, entry.controls.map(each => each.name))} label={`${entry.name} code`} data-testid="playground-code" />;
  const form = (
    <PageSection title="Props">
      <view className="gap-4">
        {fields.map(row)}
        {/* On-off props are rows of one list, as in a settings screen, so their larger labels read as a group of their own. */}
        {flags.length > 0 && <ItemList label="On or off">{flags.map(row)}</ItemList>}
        <view className="flex-row">
          <Button label="Reset" size="sm" width="content" isDisabled={edits.value[entry.name] === undefined} onClick={reset} data-testid="playground-reset" />
        </view>
      </view>
    </PageSection>
  );
  if (isWide)
    return (
      <view className="gap-8">
        <text className="text-body text-muted">Pick a component and change its props. It is redrawn as you go, and the code is what you would write.</text>
        <view className="grid grid-cols-2 gap-x-8">
          <view className="gap-4">
            {picker}
            {stage}
            {code}
          </view>
          {form}
        </view>
      </view>
    );
  return (
    <view className="grow basis-0 min-h-0 gap-3">
      {picker}
      {stage}
      <scroll className="grow basis-0 min-h-0">
        <view className="gap-8 pt-3 pb-6">
          {form}
          <PageSection title="Code">{code}</PageSection>
        </view>
      </scroll>
    </view>
  );
}
