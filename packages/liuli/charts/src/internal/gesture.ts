import type { PointerEvent } from '@lingxia/view';

// One pointer at a time, so one press's state serves every chart on the page.
const press = { x: 0, y: 0, index: -1, key: null as string | null, isSideways: false };
// How far a finger travels sideways before a chart takes it for a scrub.
const slop = 4;

/** A press landed on datum `index` while `key` was chosen. */
export function pressed(event: PointerEvent, index: number, key: string | null): void {
  Object.assign(press, { x: event.x, y: event.y, index, key, isSideways: event.pointer !== 'touch' });
}

/**
 * Whether a move belongs to the chart: a finger must first travel further sideways than up or
 * down, since the page may still take the press for a scroll.
 */
export function scrubbing(event: PointerEvent): boolean {
  const [across, along] = [Math.abs(event.x - press.x), Math.abs(event.y - press.y)];
  if (!press.isSideways && across >= slop && across > along) press.isSideways = true;
  return press.isSideways;
}

/** The datum the press landed on, until a move leaves it. */
export const pressedIndex = () => press.index;
export const leftPressed = () => { press.index = -1; };
/** What was chosen when the press landed: a press the page takes for a scroll puts it back. */
export const pressedKey = () => press.key;
