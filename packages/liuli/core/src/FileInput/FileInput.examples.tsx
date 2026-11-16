import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { FileInput, type FileInputFile } from './FileInput';

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

/** Nothing chosen, on a required field after a failed send. */
export function Empty() {
  const files = useLocal<readonly FileInputFile[]>('proof', []);
  return (
    <FileInput
      label="Proof of address"
      isRequired
      status={files.value.length === 0 ? { type: 'error', message: 'Add one document.' } : undefined}
      files={files.value}
      onPick={() => files.set([more])}
      pickLabel="Add a document"
      onRemove={() => files.set([])}
      removeLabel={name => `Remove ${name}`}
      progressLabel={name => `Uploading ${name}`}
      data-testid="proof"
    />
  );
}

export function StateDisabled() {
  return (
    <FileInput
      label="Documents"
      isDisabled
      disabledMessage="The request is closed."
      files={[first[0]]}
      onPick={unused}
      pickLabel="Choose files"
      onRemove={unused}
      removeLabel={name => `Remove ${name}`}
      progressLabel={name => `Uploading ${name}`}
    />
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <FileInput label="Files" isLabelHidden files={[]} onPick={unused} pickLabel="Choose files" onRemove={unused} removeLabel={name => `Remove ${name}`} progressLabel={name => `Uploading ${name}`} className="w-36" />
  );
}
