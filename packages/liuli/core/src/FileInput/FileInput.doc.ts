import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'FileInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Files the user chose, and a way to choose more',
  description:
    'FileInput is a wide button that asks the app to open its file picker, over a list of the files chosen so far: each row has the name, a line the app writes, a ring while it is sending, and a button that removes it. ' +
    'Liuli does not read files: picking happens in Logic, and the app owns the list. It carries Field\'s label, help text and status.',
  keywords: ['file', 'upload', 'attachment', 'picker', 'document', 'dropzone', 'browse'],
  propsType: 'FileInputProps',
  recipe: { map: 'fileInputRecipe', parts: 'fileInputParts' },
  anatomy: [
    { part: 'pick', required: true, description: 'The button that opens the app\'s picker: an icon, `pickLabel` and the hint.' },
    { part: 'hint', required: false, description: 'What may be chosen.' },
    { part: 'list', required: false, description: 'The chosen files: a list named by the label.' },
    { part: 'file', required: false, description: 'One file: its name, a line from the app, a ring while sending, a remove button.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: ['description', 'status'],
    states: [{ state: 'disabled', from: 'isDisabled' }],
    keyboard: [
      { keys: 'Tab', effect: 'To the button, then to each file\'s remove button' },
      { keys: 'Enter or Space', effect: 'Opens the picker, or removes the file' },
    ],
    target: { md: 32 },
    contrast: [
      { what: 'button boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'file name', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'failure line', pair: ['danger', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The button is named "label: pickLabel" and described by the help text and status.',
      'The files are a list; a failed file has an error icon and says why in words, not only in colour.',
      'Each ring is a progressbar named by `progressLabel`; each remove button says what it removes.',
    ],
  },
  guidance: {
    do: [
      'Say what may be chosen in `hint`: kinds and sizes.',
      'Write each file\'s line as it stands: its size, "Uploading, 64%", why it failed.',
    ],
    dont: [
      'Check kinds and sizes only after sending; say the limits first.',
      'Use it to show files that are already stored; that is an ItemList.',
    ],
  },
  examples: [
    { name: 'Documents', title: 'Documents for a request' },
    { name: 'Empty', title: 'Nothing chosen, with an error' },
  ],
  gaps: [],
  limits: [
    'Nothing can be dropped onto it: the View has no drop target.',
    'It shows no previews; pair it with Thumbnail for pictures.',
  ],
  related: ['Field', 'CircularProgress', 'Thumbnail'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Choose files',
    order: 13,
    page: {
      lead: 'Use a file input to collect files with a form.',
      hero: 'Documents',
      variants: [
        { name: 'Empty', title: 'Empty and required, with an error' },
        { name: 'StateDisabled', title: 'Disabled, with the reason' },
      ],
    },
  },
};
