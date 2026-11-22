/**
 * The `data-*` props a component was given, for the element that carries its test id: apps and
 * tests mark elements with them (`data-state`, `data-current`). JSX does not type-check hyphenated
 * props, so a component that dropped them would drop them silently. The test id stays the
 * component's own.
 */
export function dataOf(props: object): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(props)) if (key.startsWith('data-') && key !== 'data-testid') out[key] = value;
  return out;
}
