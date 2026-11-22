import { cx } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { CircularProgress } from '../CircularProgress/CircularProgress';
import { Field, type FieldOwnProps } from '../Field/Field';
import { IconButton } from '../IconButton/IconButton';
import { Glyph } from '../internal/Glyph';
import { fileInputRecipe as r } from './FileInput.recipe';
import type { TransferProgress } from '../ProgressBar/ProgressBar';
import { dataOf } from '../internal/data';

export interface FileInputFile {
  /** Identifies the file: what `onRemove` carries, and `<testid>-<id>` on its row. */
  id: string;
  name: string;
  /** A line under the name, as the app writes it: "4.2 MB", "Uploading, 64%", "Too large: 25 MB at most". */
  detail?: string;
  /** Sending: a ring at the end of the row. A number from 0 to 100, `true` while the amount is unknown, or the upload's streamed progress. */
  progress?: number | true | TransferProgress | null;
  /** Marks the row as failed; say why in `detail`. @default false */
  isFailed?: boolean;
}

export type FileInputProps = FieldOwnProps & {
  /** The chosen files, in order. The app owns the list. */
  files: readonly FileInputFile[];
  /** Opens the app\'s own picker, in Logic. Liuli cannot read files. */
  onPick: () => void;
  /** The words on the button: "Choose files", "Add a photo". Defaults to "Choose files", or "Choose a file" without `isMultiple`, in the page's language. */
  pickLabel?: string;
  /** Whether the app's picker takes several files; words the default button. @default true */
  isMultiple?: boolean;
  /** A line under them: what may be chosen. "PDF or image, up to 25 MB each". */
  hint?: string;
  /** Takes a file out of the list. */
  onRemove: (id: string) => void;
  /** Each remove button, named with what it removes: `name => \`Remove ${name}\``. Defaults to "Remove <name>" in the page's language. */
  removeLabel?: (name: string) => string;
  /** Each ring, named with what is sending: `name => \`Uploading ${name}\``. Defaults to "Uploading <name>" in the page's language. */
  progressLabel?: (name: string) => string;
  className?: string;
  /** On the button; the list gets `<testid>-files`. */
  'data-testid'?: string;
};

/** Files the user chose, with a button that opens the app\'s picker. */
export function FileInput(props: FileInputProps) {
  const { files, onPick, isMultiple = true, pickLabel = liuliText('@liuli.chooseFiles', { multiple: isMultiple ? 'yes' : 'no' }), hint, onRemove, removeLabel = (name: string) => liuliText('@liuli.removeNamed', { name }), progressLabel = (name: string) => liuliText('@liuli.uploadingNamed', { name }), className, 'data-testid': testId, ...field } = props;
  const { status, isDisabled = false } = field;
  return (
    <Field {...field} className={className}>
      {control => (
        <view className={r.root.base}>
          <button
            aria-label={`${control['aria-label']}: ${pickLabel}`}
            aria-describedby={control['aria-describedby']}
            disabled={control.disabled}
            onClick={onPick}
            data-testid={testId} {...dataOf(field)}
            className={cx(r.pick.base, status && r.pick.status[status.type])}
          >
            <Glyph name="upload" className={r.pickIcon.base} />
            <text className={r.pickLabel.base}>{pickLabel}</text>
            {hint !== undefined && <text className={r.hint.base}>{hint}</text>}
          </button>
          {files.length > 0 && (
            <view role="list" aria-label={control['aria-label']} data-testid={testId && `${testId}-files`} className={r.list.base}>
              {files.map(file => (
                <view key={file.id} role="listitem" data-testid={testId && `${testId}-${file.id}`} className={cx(r.file.base, file.isFailed && r.file.failed)}>
                  <Glyph name={file.isFailed ? 'error' : 'file'} className={r.fileIcon.base} />
                  <view className={r.fileText.base}>
                    <text className={r.name.base}>{file.name}</text>
                    {file.detail !== undefined && <text className={cx(r.detail.base, file.isFailed && r.detail.failed)}>{file.detail}</text>}
                  </view>
                  {file.progress !== undefined && (
                    <CircularProgress label={progressLabel(file.name)} {...(typeof file.progress === 'number' ? { value: file.progress } : { progress: file.progress === true ? null : file.progress })} size="sm" />
                  )}
                  <IconButton label={removeLabel(file.name)} icon="close" size="sm" isDisabled={isDisabled} onClick={() => onRemove(file.id)} data-testid={testId && `${testId}-${file.id}-remove`} />
                </view>
              ))}
            </view>
          )}
        </view>
      )}
    </Field>
  );
}
