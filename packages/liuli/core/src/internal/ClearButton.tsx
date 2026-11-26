import { liuliText } from '../i18n/text';
import { Glyph, glyphGrowth, useTextGrowth } from './Glyph';

/** A field's clear button, named "Clear" and the field's label unless the app names it. */
export function ClearButton({ label, clearLabel, onClear, testId, className, iconClassName }: {
  label: string;
  clearLabel?: string;
  onClear: () => void;
  testId?: string;
  className: string;
  iconClassName: string;
}) {
  return (
    <button aria-label={clearLabel ?? liuliText('@liuli.clearField', { label })} onClick={onClear} data-testid={testId && `${testId}-clear`} className={className}>
      <Glyph name="close" side={16} className={iconClassName} />
    </button>
  );
}

/**
 * Where a clear button lying over a button box sits once the box's trailing mark of `markSide` points has
 * grown with the text: past the box's end padding, the mark and the gap before it. Until then the dock's
 * recipe places it in theme steps.
 */
export function useClearDock(markSide: number): { insetInlineEnd: number } | undefined {
  const mark = useTextGrowth(markSide, glyphGrowth)?.width;
  return mark === undefined ? undefined : { insetInlineEnd: 12 + mark + 8 };
}
