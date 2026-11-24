// Props that exclude each other, read from a props type that is a union of alternatives, each
// barring the others' props with `?: never`.

/**
 * The sets of props that exclude each other. `names` are the props in order; `allowed` gives, for
 * each alternative, the names it lets an app pass. Props allowed in the same alternatives go
 * together (`value` with `onChange`); groups no alternative allows together exclude each other.
 * Each choice lists its groups: `[['bind'], ['value', 'onChange']]`.
 */
export function exclusiveGroups(names, allowed) {
  const where = name => allowed.map(set => (set.has(name) ? '1' : '0')).join('');
  const groups = new Map();
  for (const name of names) {
    const key = where(name);
    if (!key.includes('1') || !key.includes('0')) continue;
    groups.set(key, [...(groups.get(key) ?? []), name]);
  }
  const keys = [...groups.keys()];
  const apart = (a, b) => [...a].every((bit, index) => bit === '0' || b[index] === '0');
  const used = new Set();
  const choices = [];
  for (const key of keys) {
    if (used.has(key)) continue;
    const set = [key];
    for (const other of keys) if (!used.has(other) && !set.includes(other) && set.every(each => apart(each, other))) set.push(other);
    if (set.length < 2) continue;
    set.forEach(each => used.add(each));
    choices.push(set.map(each => groups.get(each)));
  }
  return choices;
}

/** The choices in a props type, read with the TypeScript checker; none when it is not a union. */
export function choicesOf(ts, checker, type) {
  if (!type.isUnion()) return [];
  const names = [...new Set(type.types.flatMap(member => checker.getPropertiesOfType(member).map(property => property.name)))];
  const barred = symbol => {
    const flags = checker.getTypeOfSymbol(symbol).flags;
    return (flags & ts.TypeFlags.Never) !== 0 || (flags & ts.TypeFlags.Undefined) !== 0;
  };
  const allowed = type.types.map(member => new Set(checker.getPropertiesOfType(member).filter(symbol => !barred(symbol)).map(symbol => symbol.name)));
  return exclusiveGroups(names, allowed);
}

/** A choice as one line: "`bind` | `value` + `onChange`: props from one side only". */
export function choiceLine(groups) {
  return `${groups.map(names => names.map(name => `\`${name}\``).join(' + ')).join(' | ')}: props from one side only`;
}
