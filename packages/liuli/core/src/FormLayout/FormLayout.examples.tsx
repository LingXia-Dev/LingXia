import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { Button } from '../Button/Button';
import { Select } from '../Select/Select';
import { TextArea } from '../TextArea/TextArea';
import { TextInput } from '../TextInput/TextInput';
import { FormLayout, FormSpan, type FormLayoutProps } from './FormLayout';

const countries = [{ value: 'pt', label: 'Portugal' }, { value: 'es', label: 'Spain' }, { value: 'fr', label: 'France' }];

/** A delivery address: one column on a phone, two where there is room; the street and the notes take a whole line. Every field is needed but the one that says optional. */
export function Address() {
  const first = useLocal('first', 'Ana');
  const last = useLocal('last', 'Lima');
  const street = useLocal('street', '');
  const city = useLocal('city', '');
  const post = useLocal('post', '');
  const country = useLocal('country', 'pt');
  const notes = useLocal('notes', '');
  return (
    <Said>
      {say => (
        <FormLayout
          columns={2}
          actions={<><Button label="Cancel" onClick={() => say('Cancelled')} /><Button label="Save address" variant="primary" onClick={() => say('Saved the address')} /></>}
          data-testid="address"
        >
          <TextInput label="First name" bind={first} autoComplete="given-name" isRequired hasRequiredMark={false} />
          <TextInput label="Last name" bind={last} autoComplete="family-name" isRequired hasRequiredMark={false} />
          <FormSpan><TextInput label="Street and number" bind={street} isRequired hasRequiredMark={false} data-testid="street" /></FormSpan>
          <TextInput label="City" bind={city} isRequired hasRequiredMark={false} />
          <TextInput label="Postcode" bind={post} isRequired hasRequiredMark={false} />
          <Select label="Country" bind={country} items={countries} />
          <FormSpan><TextArea label="Notes for the courier" optionalLabel="optional" bind={notes} /></FormSpan>
        </FormLayout>
      )}
    </Said>
  );
}

/** A profile where little is needed: the one required field carries the mark, and nothing says optional. */
export function Profile() {
  const name = useLocal('name', '');
  const site = useLocal('site', '');
  const about = useLocal('about', '');
  return (
    <Said>
      {say => (
        <FormLayout actions={<Button label="Save profile" variant="primary" onClick={() => say('Saved the profile')} />} data-testid="profile">
          <TextInput label="Display name" bind={name} isRequired data-testid="display-name" />
          <TextInput label="Website" bind={site} placeholder="example.com" />
          <TextArea label="About you" bind={about} />
        </FormLayout>
      )}
    </Said>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <FormLayout className="w-32">
      <view className="h-6 rounded-control border border-line-strong" />
      <view className="h-6 rounded-control border border-line-strong" />
    </FormLayout>
  );
}

/** The playground's props that are not plain values. */
export function playground() {
  return {
    columns: 2,
    children: [
      <TextInput key="first" label="First name" autoComplete="given-name" value="" onChange={unused} />,
      <TextInput key="last" label="Last name" autoComplete="family-name" value="" onChange={unused} />,
      <FormSpan key="street"><TextInput label="Street and number" value="" onChange={unused} /></FormSpan>,
    ],
    actions: (
      <>
        <Button label="Cancel" onClick={unused} />
        <Button label="Save address" variant="primary" onClick={unused} />
      </>
    ),
  } satisfies Partial<FormLayoutProps>;
}
