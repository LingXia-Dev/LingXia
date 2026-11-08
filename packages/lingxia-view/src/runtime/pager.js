// `<Pager>`: pages side by side in a horizontal scroll that stops on each one, with dots under it.
// The pages are a list, so a screen reader reads "n of m" on the page it lands on; the dots repeat
// that for the eye and stay out of the accessibility tree.
import { h, childrenToArray } from './nodes.js';
import { useLocal } from './hooks.js';

const dotMotion = { properties: ['layout', 'bg'], durationMs: 200, delayMs: 0, easing: [0.4, 0, 0.2, 1] };

export function Pager({ bind, defaultPage = 0, dots = true, 'aria-label': label, className, 'data-testid': testId, children }) {
  if (bind !== undefined && (typeof bind?.set !== 'function' || typeof bind.value !== 'number'))
    throw Error('UI_PROP: bind must be a Local<number>');
  if (typeof label !== 'string' || !label) throw Error('UI_PROP: aria-label is required');
  const local = useLocal('page', defaultPage);
  const page = bind ?? local;
  const pages = childrenToArray(children).filter(child => child !== null && child !== undefined && child !== false);
  const at = Math.min(Math.max(0, Math.round(page.value)), Math.max(0, pages.length - 1));
  return h('column', { className, 'data-testid': testId, style: { gap: 8 } },
    h('scroll', { bind: page, 'data-testid': testId && `${testId}-scroll`,
      style: { overflowX: 'auto', scrollSnapType: 'x', scrollSnapStop: 'always' } },
      h('row', { role: 'list', 'aria-label': label },
        ...pages.map((content, index) => h('column', { key: String(index), role: 'listitem',
          'data-testid': testId && `${testId}-page-${index}`,
          style: { width: '100%', flexShrink: 0, scrollSnapAlign: 'start' } }, content)))),
    dots && pages.length > 1 && h('row', { 'aria-hidden': true, 'data-testid': testId && `${testId}-dots`,
      style: { justifyContent: 'center', alignItems: 'center' } },
      // A finger-sized target around each small mark.
      ...pages.map((_, index) => h('button', { key: String(index), 'data-testid': testId && `${testId}-dot-${index}`,
        onClick: () => page.set(index),
        style: { paddingTop: 9, paddingBottom: 9, paddingLeft: 3, paddingRight: 3, backgroundColor: 'transparent' } },
        h('column', { 'data-testid': testId && `${testId}-mark-${index}`,
          style: { width: index === at ? 18 : 6, height: 6, borderRadius: 3,
            backgroundColor: index === at ? 'accent' : 'line', transition: dotMotion } })))));
}
Pager.__lxid = '@lingxia/view#Pager';
