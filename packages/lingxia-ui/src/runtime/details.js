// `<Details>`/`<Summary>`: a disclosure, as HTML's details/summary. The summary is a button
// announcing its expanded state; the body exists only while open.
import { h, isNode, readNode, childrenToArray } from './nodes.js';
import { useLocal } from './hooks.js';

/** The summary row of a `<Details>`: its children label the toggle. */
export function Summary({ children }) { return children ?? null; }
Summary.__lxid = '@lingxia/ui#Summary';

const chevron = [{ d: 'M9 6l6 6-6 6', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' }];

export function Details({ open, defaultOpen = false, onToggle, className, testId, children }) {
  if (open !== undefined && typeof open !== 'boolean') throw Error('UI_PROP: open must be boolean');
  if (onToggle !== undefined && typeof onToggle !== 'function') throw Error('UI_PROP: onToggle');
  const local = useLocal('open', defaultOpen === true);
  const expanded = open ?? local.value;
  const items = childrenToArray(children);
  const at = items.findIndex(item => isNode(item) && readNode(item)[0] === Summary);
  const summary = at < 0 ? null : readNode(items[at]);
  const label = summary ? childrenToArray(summary[2].length ? summary[2] : summary[1].children) : ['Details'];
  const body = items.filter((_, index) => index !== at);
  const toggle = () => {
    if (open === undefined) local.set(!expanded);
    onToggle?.(!expanded);
  };
  return h('column', { className, testId },
    h('button', { testId: testId && `${testId}-summary`, onClick: toggle, accessibilityState: { expanded },
      className: summary?.[1].className, style: summary?.[1].className ? undefined : { backgroundColor: 'transparent', color: 'text', paddingTop: 8, paddingBottom: 8 } },
      h('row', { style: { alignItems: 'center', gap: 8 } },
        h('vector', { accessibilityHidden: true, paths: chevron,
          style: { width: 16, height: 16, rotate: expanded ? 90 : 0, transition: { properties: ['transform'], durationMs: 150, delayMs: 0, easing: [0.4, 0, 0.2, 1] } } },
        ),
        ...label.map(item => typeof item === 'string' || typeof item === 'number' ? h('text', {}, item) : item))),
    expanded && h('column', { testId: testId && `${testId}-body` }, ...body));
}
Details.__lxid = '@lingxia/ui#Details';
