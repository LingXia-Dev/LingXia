const directions = ['top', 'right', 'bottom', 'left', 'top-right', 'bottom-right', 'bottom-left', 'top-left'];

export function normalizeGradient(value) {
  if (value === 'none') return value;
  const invalid = () => { throw Error('UI_GRADIENT: invalid linear gradient'); };
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      Object.keys(value).some(key => !['direction', 'angle', 'colorSpace', 'stops'].includes(key)) ||
      (value.direction !== undefined && !directions.includes(value.direction)) ||
      (value.angle !== undefined && (typeof value.angle !== 'number' || !Number.isFinite(value.angle) || Math.abs(value.angle) > 36000)) ||
      (value.direction !== undefined && value.angle !== undefined) ||
      (value.colorSpace !== undefined && !['srgb', 'oklab'].includes(value.colorSpace)) ||
      !Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 16) invalid();
  let previous = -Infinity;
  const stops = value.stops.map(stop => {
    if (!stop || typeof stop !== 'object' || Array.isArray(stop) ||
        Object.keys(stop).some(key => key !== 'offset' && key !== 'color') ||
        typeof stop.offset !== 'number' || !Number.isFinite(stop.offset) || Math.abs(stop.offset) > 16 || stop.offset < previous ||
        typeof stop.color !== 'string' || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(stop.color)) invalid();
    previous = stop.offset;
    return { offset: stop.offset, color: stop.color };
  });
  return { ...(value.direction !== undefined ? {direction: value.direction} : {}),
    ...(value.angle !== undefined ? {angle: value.angle} : {}), colorSpace: value.colorSpace ?? 'srgb', stops };
}
