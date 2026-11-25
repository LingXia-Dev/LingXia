import { cx, useContext, type Child } from '@lingxia/view';
import type { IconRef } from '../Icon/Icon';
import type { ToggleButtonSize } from '../ToggleButton/ToggleButton';
import { toggleButtonRecipe as r } from '../ToggleButton/ToggleButton.recipe';
import { Glyph, glyphGrowth, useTextGrowth } from './Glyph';
import { LayerContext } from './layer';

const glyphSides = { sm: 16, md: 18, lg: 20 } satisfies Record<ToggleButtonSize, number>;
const squareSides = { sm: 32, md: 40, lg: 48 } satisfies Record<ToggleButtonSize, number>;

/**
 * A toggle's face and box, shared with ToggleButtonGroup: the classes for its state, the square an
 * icon-only toggle keeps as its glyph grows with text, and what it draws.
 */
export function useToggleFace(look: { label: string; size: ToggleButtonSize; isOn: boolean; icon?: IconRef; pressedIcon?: IconRef; isLabelHidden?: boolean }) {
  const { label, size, isOn, icon, pressedIcon, isLabelHidden } = look;
  const layer = useContext(LayerContext);
  const glyph = useTextGrowth(glyphSides[size], glyphGrowth);
  const side = isLabelHidden && glyph ? squareSides[size] + glyph.width - glyphSides[size] : undefined;
  const shown = isOn && pressedIcon !== undefined ? pressedIcon : icon;
  const content: Child = (
    <>
      {shown !== undefined && <Glyph name={shown} side={glyphSides[size]} className={r.glyph.size[size]} />}
      {!isLabelHidden && <text className={cx(r.label.base, r.label.size[size])}>{label}</text>}
    </>
  );
  return {
    className: cx(r.root.base, isOn ? r.root.on : cx(r.root.off, layer === 'overlay' && r.root.overlay), isLabelHidden ? r.root.square[size] : r.root.size[size]),
    style: side === undefined ? undefined : { width: side, height: side },
    content,
  };
}

