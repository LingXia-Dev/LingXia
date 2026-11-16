<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# FormLayout

A form's fields at one rhythm: a column on a phone, two where there is room. Status: preview.

```ts
import { FormLayout, FormSpan } from '@liulidesign/core/FormLayout';
```

FormLayout sets the distance between a form's fields and puts its buttons after them. With `columns={2}` the fields pair up where the form is at least 560 points wide and stay in one column below that; FormSpan, from the same import, gives a field the whole line. It lays fields out and nothing more: each field keeps its own label, help and status.

## Examples

### A delivery address

```tsx
import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { Select } from '@liulidesign/core/Select';
import { TextArea } from '@liulidesign/core/TextArea';
import { TextInput } from '@liulidesign/core/TextInput';
import { FormLayout, FormSpan } from '@liulidesign/core/FormLayout';

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
          <Select label="Country" bind={country} options={countries} />
          <FormSpan><TextArea label="Notes for the courier" optionalLabel="optional" bind={notes} /></FormSpan>
        </FormLayout>
      )}
    </Said>
  );
}
```

### Mostly optional, the required field starred

```tsx
import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { TextArea } from '@liulidesign/core/TextArea';
import { TextInput } from '@liulidesign/core/TextInput';
import { FormLayout } from '@liulidesign/core/FormLayout';

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
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  | The fields, in reading order; a `FormSpan` takes a whole line. |
| `columns` | `1 \| 2` | `1` | The most columns the fields may take, where the form is wide enough for them. |
| `actions` | `Child` |  | The buttons that submit or leave the form, drawn after the fields. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## FormSpan props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  |  |
| `className` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The fields, then the actions. |
| fields | `<view>` | yes | A column, or a two-column grid where the form is wide enough. |
| span | `<view>` | no | FormSpan: a field that takes the whole line. |
| actions | `<view>` | no | The form's buttons, at the end edge; they wrap rather than shrink. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on FormSpan: classes merged after the recipe's, so they win.
- The recipe, `formLayoutRecipe` from `@liulidesign/core/FormLayout/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Tab: Through the fields in the order they are written, across a line before down, then the actions.
- Press target: .
- The reading and Tab order is the order the fields are written in, in one column and in two.
- It adds no role: the page's heading names the form, and each field names itself.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write the fields in the order people fill them in; pair only fields that belong together, such as first and last name.
- Do: Give long fields a whole line with FormSpan: an address line, a note.
- Do: Put the primary action last, at the end edge.
- Do: Mark the fewer kind: where most fields are needed, label the optional ones; where most are optional, star the required ones.
- Don't: Use two columns for a long form of unrelated fields; one column is filled in faster.
- Don't: Put a heading inside it for one or two fields; use Section around groups of fields.
- Don't: Star required fields and label optional ones in the same form.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Actions that stack full-width on a phone. | nothing from the View | The buttons sit at the end edge and wrap. |
| More than two columns, and fields that span some of them. | nothing from the View | One or two columns; a field takes one or the whole line. |

Related: Field, TextInput, Section.
