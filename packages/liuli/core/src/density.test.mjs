import { test } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { renderPage } from './testing/harness.mjs';

const page = linked => {
  const from = name => JSON.stringify(join(linked, 'src', name));
  return `import { DensityProvider } from ${from('density')};
import { TextInput } from ${from('TextInput/TextInput')};
import { Select } from ${from('Select/Select')};
import { Item, ItemList } from ${from('Item/Item')};

function Form({ id }: { id: string }) {
  return (
    <view>
      <TextInput label="Name" value="" onChange={() => {}} data-testid={id + '-name'} />
      <TextInput label="City" value="" onChange={() => {}} size="lg" data-testid={id + '-city'} />
      <Select label="Country" value="" onChange={() => {}} items={[{ value: 'pt', label: 'Portugal' }]} data-testid={id + '-country'} />
      <ItemList label="Notes"><Item label="Trip to the coast" data-testid={id + '-row'} /></ItemList>
    </view>
  );
}

export function render() {
  return (
    <page>
      <Form id="plain" />
      <DensityProvider value="compact"><Form id="dense" /></DensityProvider>
    </page>
  );
}
`;
};

test('DensityProvider: compact makes Field controls small and rows compact; a control\'s own size still wins', async () => {
  const screen = await renderPage(page);
  const boxHeight = id => screen.getByTestId(id).parent.props.minHeight;
  assert.deepEqual([boxHeight('plain-name'), boxHeight('dense-name')], [40, 32]);
  assert.equal(boxHeight('dense-city'), 48, 'its own size wins');
  assert.ok(screen.getByTestId('dense-country').props.minHeight < screen.getByTestId('plain-country').props.minHeight);
  assert.ok(screen.getByTestId('dense-row').props.minHeight < screen.getByTestId('plain-row').props.minHeight);
});
