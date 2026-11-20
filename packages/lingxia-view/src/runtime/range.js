// A slider with two thumbs: a named group of two sliders over one track. Each is a slider of its
// own to the keyboard and to assistive tech; the host keeps them from crossing and gives a press
// to the nearer thumb.
import { h } from './nodes.js';
import { partitionClasses } from '../style/classes.js';
import { slider as checkSlider } from './validate.js';

/** On a slider that is one thumb of a pair: `{ other, upper }`. */
export const thumbMark = Symbol('slider thumb');

// What the control draws with stays on the sliders; what places and sizes it goes to the group.
const drawn = key => key === 'color' || key.startsWith('padding');
const tall = key => key === 'height' || key === 'minHeight' || key === 'maxHeight';
const keys = rule => Object.keys(rule.style ?? {});
const pick = (style, test) => Object.fromEntries(Object.entries(style ?? {}).filter(([key]) => test(key)));
const over = { position: 'absolute', top: 0, left: 0, right: 0 };

export function rangeSlider(props) {
  const { values, thumbLabels, onChange, onChangeEnd, onPressChange, className, style, min, max, step, disabled, key, id, role,
    'data-testid': testId, 'aria-label': ariaLabel, accessibilityLabel = ariaLabel, ...rest } = props;
  for (const name of ['value', 'bind', 'defaultValue', 'secondaryValue', 'aria-valuetext']) if (rest[name] !== undefined) throw Error(`UI_SLIDER: ${name} belongs to a slider with one value`);
  if (role !== undefined) throw Error('UI_SLIDER: a slider with two values is a group of two sliders; it takes no role');
  if (!Array.isArray(values) || values.length !== 2 || !values.every(value => typeof value === 'number' && Number.isFinite(value)) || values[0] > values[1])
    throw Error('UI_SLIDER: values is [lower, upper], two numbers in order');
  if (!Array.isArray(thumbLabels) || thumbLabels.length !== 2 || !thumbLabels.every(label => typeof label === 'string' && label.trim()))
    throw Error('UI_A11Y: a slider with two values needs a name for each thumb; add thumbLabels={["From", "To"]}');
  if (typeof accessibilityLabel !== 'string' || !accessibilityLabel.trim()) throw Error('UI_A11Y: <slider> needs an accessible name; add aria-label="…"');
  for (const [name, handler] of [['onChange', onChange], ['onChangeEnd', onChangeEnd]])
    if (handler !== undefined && typeof handler !== 'function') throw Error(`UI_SLIDER: ${name} takes a function`);
  // Each end snaps as a slider's value does, so the two meet on the same grid.
  const snap = value => { const one = { min, max, step, value, accessibilityLabel }; checkSlider(one); return one.value; };
  const [lower, upper] = [snap(values[0]), snap(values[1])];
  const [own, placed] = partitionClasses(className, rule => rule.when?.part || rule.when?.state || keys(rule).some(drawn));
  const [height] = partitionClasses(placed, rule => keys(rule).some(tall));
  const thumb = (at, label, value, other, extra, ownStyle) => h('slider', {
    ...rest, key: at, min, max, step, disabled, value, 'aria-label': label, 'data-testid': testId && `${testId}-${at}`,
    className: [own, extra].filter(Boolean).join(' ') || undefined, style: ownStyle,
    [thumbMark]: { other, upper: at === 'upper' },
    onChange: onChange && (next => onChange(at === 'upper' ? [lower, next] : [next, upper])),
    onChangeEnd: onChangeEnd && (next => onChangeEnd(at === 'upper' ? [lower, next] : [next, upper])),
    onPressChange,
  });
  // The lower slider is in the flow and gives the group its height; the upper lies over it at
  // the same height, so the two thumbs ride one line.
  const sized = pick(style, key => drawn(key) || tall(key));
  const lowerThumb = thumb('lower', thumbLabels[0], lower, upper, height, sized);
  const upperThumb = thumb('upper', thumbLabels[1], upper, lower, height, { ...sized, ...over });
  return {
    node: h('view', { key, id, role: 'group', 'aria-label': accessibilityLabel, 'data-testid': testId, className: placed || undefined, style: pick(style, key => !drawn(key)) },
      lowerThumb, upperThumb),
    finish() {},
  };
}
