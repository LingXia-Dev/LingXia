import { buttonProps, reachProps, imageProps, inputProps, checkProps, radioProps, sliderProps, gridProps, textProps, plotProps } from '../schema.js';

const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const common = new Set(['key', 'testId', 'style', 'className', 'children']);
const extra = {
  list: ['count', 'row', 'height', 'anchor', 'itemKeys'],
  vector: ['paths', 'viewBox', 'fit'],
  input: ['bind', 'enabled', 'accessibilityLabel'], textarea: ['bind', 'enabled', 'accessibilityLabel'], search: ['bind', 'enabled', 'accessibilityLabel'],
  checkbox: ['bind', 'enabled', 'accessibilityLabel'], switch: ['bind', 'enabled'],
  slider: ['bind', 'enabled'], radio: ['options', 'value', 'bind', 'enabled', 'onChange'],
};
const wireProps = {
  button: buttonProps, text: textProps, grid: gridProps, plot: plotProps,
  image: imageProps, input: inputProps, textarea: inputProps, search: inputProps,
  checkbox: checkProps, switch: checkProps, slider: sliderProps, radio: radioProps,
};

// Compiler and runtime share author-facing names; native wire validation stays
// independent. Children are consumed by h before native records are produced.
export function acceptsProp(tag, key) {
  if (tag === 'search' && ['secure', 'keyboard'].includes(key)) return false;
  return common.has(key) || extra[tag]?.includes(key) ||
    (wireProps[tag] !== undefined && has(wireProps[tag], key)) ||
    (['scroll', 'list'].includes(tag) && has(reachProps, key)) || false;
}
