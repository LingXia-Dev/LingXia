<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# FileInput

Files the user chose, and a way to choose more. Status: preview.

```ts
import { FileInput } from '@liulidesign/core/FileInput';
```

FileInput is a wide button that asks the app to open its file picker, over a list of the files chosen so far: each row has the name, a line the app writes, a ring while it is sending, and a button that removes it. Liuli does not read files: picking happens in Logic, and the app owns the list. It carries Field's label, help text and status.

## Examples

### Documents for a request

```tsx
import { useLocal } from '@lingxia/view';
import { FileInput, type FileInputFile } from '@liulidesign/core/FileInput';

const first: FileInputFile[] = [
  { id: 'lease', name: 'Lease agreement.pdf', detail: '1.8 MB' },
  { id: 'meter', name: 'Meter reading, November.jpg', detail: 'Uploading, 64%', progress: 64 },
  { id: 'scan', name: 'Passport scan.tiff', detail: 'Too large: 25 MB at most', isFailed: true },
];
const more: FileInputFile = { id: 'bill', name: 'Electricity bill.pdf', detail: 'Waiting to upload', progress: true };

/** Documents for a request: the app opens its picker and owns the list; each row says how its file stands. */
export function Documents() {
  const files = useLocal<readonly FileInputFile[]>('files', first);
  return (
    <FileInput
      label="Documents"
      description="We keep them until the request is closed."
      files={files.value}
      onPick={() => { if (!files.value.some(file => file.id === more.id)) files.set([...files.value, more]); }}
      pickLabel="Choose files"
      hint="PDF or image, up to 25 MB each"
      onRemove={id => files.set(files.value.filter(file => file.id !== id))}
      removeLabel={name => `Remove ${name}`}
      progressLabel={name => `Uploading ${name}`}
      data-testid="documents"
    />
  );
}
```

### Nothing chosen, with an error

```tsx
import { FileInput } from '@liulidesign/core/FileInput';

/** Nothing chosen, on a required field after a failed send. */
export function Empty() {
  return (
    <FileInput
      label="Proof of address"
      isRequired
      status={{ type: 'error', message: 'Add one document.' }}
      files={[]}
      onPick={() => {}}
      pickLabel="Add a document"
      onRemove={() => {}}
      removeLabel={name => `Remove ${name}`}
      progressLabel={name => `Uploading ${name}`}
      data-testid="proof"
    />
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The visible label and the control's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Hides the visible label; the control keeps it as its name. |
| `description` | `string` |  | Help text under the control, read as the control's description. |
| `status` | `FieldStatus` |  | An `error` status marks the control invalid; its message is read as the control's description. |
| `isRequired` | `boolean` | `false` | Marks the label with an asterisk and the control as required. Validation is the app's. |
| `hasRequiredMark` | `boolean` | `true` | Draws the asterisk on a required label; turn it off on a form where every field is required. |
| `optionalLabel` | `string` |  | A word after the label for a field that may be left empty, as the app writes it: "optional". Read before the help text. |
| `info` | `FieldInfo` |  | An info button after the label that opens a note. For what most people need, use `description`: it is always in view. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |
| `files` (required) | `readonly FileInputFile[]` |  | The chosen files, in order. The app owns the list. |
| `onPick` (required) | `() => void` |  | Opens the app\'s own picker, in Logic. Liuli cannot read files. |
| `pickLabel` (required) | `string` |  | The words on the button: "Choose files", "Add a photo". |
| `hint` | `string` |  | A line under them: what may be chosen. "PDF or image, up to 25 MB each". |
| `onRemove` (required) | `(id: string) => void` |  | Takes a file out of the list. |
| `removeLabel` (required) | `(name: string) => string` |  | Each remove button, named with what it removes: `name => \`Remove ${name}\``. |
| `progressLabel` (required) | `(name: string) => string` |  | Each ring, named with what is sending: `name => \`Uploading ${name}\``. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the button; the list gets `<testid>-files`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| pick | `<button>` | yes | The button that opens the app's picker: an icon, `pickLabel` and the hint. |
| hint | `<text>` | no | What may be chosen. |
| list | `<view>` | no | The chosen files: a list named by the label. |
| file | `<view>` | no | One file: its name, a line from the app, a ring while sending, a remove button. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: description, then status.
- States: undefined (from undefined).
- Tab: To the button, then to each file's remove button.
- Enter or Space: Opens the picker, or removes the file.
- Press target: md 32 px.
- The button is named "label: pickLabel" and described by the help text and status.
- The files are a list; a failed file has an error icon and says why in words, not only in colour.
- Each ring is a progressbar named by `progressLabel`; each remove button says what it removes.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| button boundary | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| file name | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| failure line | `danger` on `surface` | 4.5:1 | 5.21:1 | 5.42:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say what may be chosen in `hint`: kinds and sizes.
- Do: Write each file's line as it stands: its size, "Uploading, 64%", why it failed.
- Don't: Check kinds and sizes only after sending; say the limits first.
- Don't: Use it to show files that are already stored; that is an ItemList.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |
| undefined | undefined | undefined |

Related: Field, CircularProgress, Thumbnail.
