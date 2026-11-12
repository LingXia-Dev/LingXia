/** Joins class text: strings and numbers, arrays of them, and `{ 'class names': condition }` maps;
 * falsy values drop out. The compiler checks every class it can see in the arguments. */
export function cx(...values) {
  const out = [];
  const add = value => {
    if (!value) return;
    if (typeof value === 'string' || typeof value === 'number') out.push(String(value));
    else if (Array.isArray(value)) value.forEach(add);
    else if (typeof value === 'object') for (const key of Object.keys(value)) if (value[key]) out.push(key);
  };
  values.forEach(add);
  return out.join(' ');
}
