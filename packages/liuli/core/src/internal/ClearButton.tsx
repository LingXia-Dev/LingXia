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
 * Where a clear button lying over a button box sits: before the box's trailing mark of `markSide`
 * points, which grows with the text, past the box's end padding and the gap before the mark.
 */
export function useClearDock(markSide: number): { insetInlineEnd: number } {
  const mark = useTextGrowth(markSide, glyphGrowth)?.width ?? markSide;
  return { insetInlineEnd: 12 + mark + 8 };
}
