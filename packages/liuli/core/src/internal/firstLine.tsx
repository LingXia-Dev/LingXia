import { cx, type Box, type Child } from '@lingxia/view';
import { firstLineRecipe as r } from './firstLine.recipe';

/**
 * Leading content (an icon, a tile, an avatar) centred on the first line of the text beside it, at any text size.
 * An invisible strut in `strut`, the first line's text style, sets the line's height; content taller than the line
 * aligns with the text's top instead, so it never rises above it. `box` measures one line.
 */
export function FirstLine({ strut, box, isCentred = false, className, children, ...rest }: {
  strut: string; box?: Box;
  /** Centre on the whole text instead, as a short row does. */
  isCentred?: boolean;
  className?: string; children: Child; 'aria-hidden'?: boolean;
}) {
  return (
    <view aria-hidden={rest['aria-hidden']} className={cx(r.root.base, r.root.along[isCentred ? 'centre' : 'first'], className)}>
      <text box={box} aria-hidden className={cx(strut, r.strut)}>{'\u200B'}</text>
      {children}
    </view>
  );
}
