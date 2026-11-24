import { elements, blocks, limits, style } from '../generated/contract.js';

/** A unitless lineHeight past the multiplier cap: almost always points written without a unit. */
export function lineHeightProblem(value) {
  if (typeof value !== 'number' || !(value > limits.lineHeightMultiplier)) return undefined;
  return `UI_STYLE_VALUE: lineHeight ${value} is a multiple of the font size (${value}×), not points; write "${value}px", or a leading-* class`;
}

const unsignedStyleNumber = new Set(['dimension', 'length', 'number', 'basis']);
const signedStyleNumber = new Set(['margin', 'inset']);

/** Build-time range check for a numeric style literal governed by `limits.styleNumber` or `limits.lineHeightMultiplier`. */
export function styleValueProblem(name, value) {
  const entry = style[name];
  if (!entry || typeof value !== 'number' || !Number.isFinite(value)) return undefined;
  if (entry.lower === 'line-height') {
    if (value > limits.lineHeightMultiplier) return lineHeightProblem(value);
    if (value <= 0) return `UI_STYLE_VALUE: ${name} takes a multiplier above 0 up to ${limits.lineHeightMultiplier}, or <n>px; got ${value}`;
    return undefined;
  }
  if (entry.lower === 'font-size') {
    if (value <= 0) return `UI_STYLE_VALUE: ${name} takes a number above 0; got ${value}`;
    if (value > limits.styleNumber) return `UI_STYLE_VALUE: ${name} takes a number from 0 to ${limits.styleNumber}; got ${value}`;
    return undefined;
  }
  if (unsignedStyleNumber.has(entry.lower)) {
    if (value < 0 || value > limits.styleNumber) return `UI_STYLE_VALUE: ${name} takes a number from 0 to ${limits.styleNumber}; got ${value}`;
    return undefined;
  }
  if (signedStyleNumber.has(entry.lower)) {
    if (value < -limits.styleNumber || value > limits.styleNumber) return `UI_STYLE_VALUE: ${name} takes a number from ${-limits.styleNumber} to ${limits.styleNumber}; got ${value}`;
    return undefined;
  }
  return undefined;
}

const container = new Set(['alignItems', 'alignContent', 'justifyContent', 'justifyItems', 'flexDirection', 'flexWrap',
  'gap', 'rowGap', 'columnGap', 'gridTemplateColumns', 'gridTemplateRows', 'gridAutoColumns', 'gridAutoRows', 'gridAutoFlow']);
const layouts = new Set(['view', 'scroll', 'list', 'button', 'checkbox', 'page']);
// HTML tags the View lacks, and what replaces each.
const boxes = { column: '<view>', row: '<view className="flex-row">', grid: '<view className="grid">',
  stack: '<view className="grid *:col-start-1 *:row-start-1">', spacer: '<view>', iframe: '<webview>',
  details: '<button aria-expanded> and a body shown from useLocal state', summary: '<button aria-expanded> and a body shown from useLocal state',
  progress: '<view role="progressbar" aria-valuenow>' };

/** `; use …` for an HTML tag the View replaces, else nothing. */
export function tagHint(tag) {
  return Object.hasOwn(boxes, tag) ? `; use ${boxes[tag]}` : '';
}

/** What a misspelt prop is written as: `testId` and friends, camel-case `data*` attributes. */
export function propHint(prop) {
  if (/^(data-?)?test-?id$/i.test(prop)) return '; a test id is data-testid (`id` names popover and aria targets)';
  if (/^data[A-Z]/.test(prop)) return `; data attributes are written ${prop.replace(/[A-Z]/g, c => `-${c.toLowerCase()}`)}`;
  return '';
}

// Flex-only properties a grid refuses, with the grid way to say it.
const gridOnly = {
  flexWrap: '; grid items wrap into rows by themselves (use grid-cols-[repeat(auto-fit,minmax(…,1fr))])',
  flexDirection: '; use grid-flow-col to fill columns first',
};

/** The flex-only property among `props` that a grid refuses, if any. */
export const gridRefuses = props => props.find(prop => Object.hasOwn(gridOnly, prop));

/** `UI_CLASS_ELEMENT` for class `name` setting flex-only `prop` on a grid. */
export function gridProblem(name, prop) {
  return `UI_CLASS_ELEMENT: ${name} does not apply to <view className="grid">${gridOnly[prop]}`;
}

/** `UI_CLASS_ELEMENT` for class `name` setting `prop`, which `<tag>` does not render. */
export function classElementProblem(name, prop, tag) {
  let hint = '';
  // Flex elements center with the flex pair; `place-items-*` and `justify-items-*` are grid-only.
  if (prop === 'justifyItems' && layouts.has(tag)) hint = '; on a flex element center with items-center justify-center';
  else if (container.has(prop) && !layouts.has(tag)) hint = '; layout classes go on the element that holds it';
  else hint = appliesTo(prop);
  return `UI_CLASS_ELEMENT: ${name} does not apply to <${blocks[tag]?.of ?? tag}>${hint}`;
}

/** `; it applies to <a>, <b>` when only a few elements render `prop`, else nothing. */
export function appliesTo(prop) {
  const takers = Object.keys(elements).filter(name => elements[name].style.includes(prop));
  return takers.length && takers.length <= 3 ? `; it applies to ${takers.map(name => `<${elements[name].component ?? name}>`).join(', ')}` : '';
}
