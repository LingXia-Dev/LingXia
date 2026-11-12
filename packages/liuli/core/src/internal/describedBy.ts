/** The `aria-describedby` value for the ids present, in reading order; undefined when there are none. */
export function describedBy(...ids: ReadonlyArray<string | false | undefined>): string | undefined {
  const present = ids.filter((id): id is string => typeof id === 'string' && id !== '');
  return present.length ? present.join(' ') : undefined;
}
