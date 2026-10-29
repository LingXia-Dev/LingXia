import { elements } from '../generated/contract.js';

// The fix for a class that sets a property its element does not render.
const grid = {
  flexWrap: '; grid items wrap into rows by themselves (use grid-cols-[repeat(auto-fit,minmax(…,1fr))])',
  flexDirection: '; use grid-flow-col to fill columns first',
};
const container = new Set(['alignItems', 'alignContent', 'justifyContent', 'justifyItems', 'flexDirection', 'flexWrap',
  'gap', 'rowGap', 'columnGap', 'gridTemplateColumns', 'gridTemplateRows', 'gridAutoColumns', 'gridAutoRows', 'gridAutoFlow']);
const layouts = new Set(['view', 'column', 'row', 'grid', 'scroll', 'list', 'stack', 'button', 'checkbox', 'page']);

/** `UI_CLASS_ELEMENT` for class `name` setting `prop`, which `<tag>` does not render. */
export function classElementProblem(name, prop, tag) {
  let hint = '';
  if (tag === 'grid') hint = grid[prop] ?? '';
  // Flex elements center with the flex pair; `place-items-*` and `justify-items-*` are grid-only.
  else if (prop === 'justifyItems' && layouts.has(tag)) hint = '; on a flex element center with items-center justify-center';
  else if (container.has(prop) && !layouts.has(tag)) hint = '; layout classes go on the element that holds it';
  else hint = appliesTo(prop);
  return `UI_CLASS_ELEMENT: ${name} does not apply to <${tag}>${hint}`;
}

/** `; it applies to <a>, <b>` when only a few elements render `prop`, else nothing. */
export function appliesTo(prop) {
  const takers = Object.keys(elements).filter(name => elements[name].style.includes(prop) && name !== 'column');
  return takers.length && takers.length <= 3 ? `; it applies to ${takers.map(name => `<${name}>`).join(', ')}` : '';
}
