export function normalizeShadows(value) {
  if (value === 'none') return value;
  const invalid = () => { throw Error('UI_SHADOW: invalid box shadow'); };
  if (!Array.isArray(value) || value.length > 8) invalid();
  return value.map(shadow => {
    if (!shadow || typeof shadow !== 'object' || Array.isArray(shadow) ||
        Object.keys(shadow).some(key => !['offsetX', 'offsetY', 'blur', 'spread', 'color'].includes(key))) invalid();
    const result = {};
    for (const key of ['offsetX', 'offsetY', 'blur', 'spread']) {
      const number = shadow[key] === undefined ? 0 : shadow[key];
      if (typeof number !== 'number' || !Number.isFinite(number) ||
          (key === 'blur' ? number < 0 || number > 256 : Math.abs(number) > 1024)) invalid();
      result[key] = number;
    }
    result.color = shadow.color === undefined ? 'currentColor' : shadow.color;
    if (result.color !== 'currentColor' && (typeof result.color !== 'string' || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(result.color))) invalid();
    return result;
  });
}
