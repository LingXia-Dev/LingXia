import { isRole } from './color.js';

const hex = /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/;
const directions = ['top', 'right', 'bottom', 'left', 'top-right', 'bottom-right', 'bottom-left', 'top-left'];
const geometry = { linear: ['direction', 'angle'], radial: ['shape', 'size', 'at'], conic: ['from', 'at'] };
const sizes = ['closest-side', 'closest-corner', 'farthest-side', 'farthest-corner'];
const angle = value => typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 36000;

export function normalizeGradient(value) {
  if (value === 'none') return value;
  const invalid = () => { throw Error('UI_GRADIENT: invalid gradient'); };
  if (!value || typeof value !== 'object' || Array.isArray(value)) invalid();
  const kind = value.kind ?? 'linear';
  const keys = geometry[kind] ?? invalid();
  if (Object.keys(value).some(key => !['kind', 'colorSpace', 'stops', ...keys].includes(key)) ||
      (value.direction !== undefined && !directions.includes(value.direction)) ||
      (value.angle !== undefined && !angle(value.angle)) ||
      (value.direction !== undefined && value.angle !== undefined) ||
      (value.from !== undefined && !angle(value.from)) ||
      (value.shape !== undefined && !['circle', 'ellipse'].includes(value.shape)) ||
      (value.size !== undefined && !sizes.includes(value.size)) ||
      (value.at !== undefined && (!Array.isArray(value.at) || value.at.length !== 2 ||
        value.at.some(v => typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v) > 16))) ||
      (value.colorSpace !== undefined && !['srgb', 'oklab'].includes(value.colorSpace)) ||
      !Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 16) invalid();
  // Radial and conic stops have no box line to extend beyond.
  const [low, high] = kind === 'linear' ? [-16, 16] : kind === 'radial' ? [0, 16] : [0, 1];
  let previous = -Infinity;
  const stops = value.stops.map(stop => {
    if (!stop || typeof stop !== 'object' || Array.isArray(stop) ||
        Object.keys(stop).some(key => key !== 'offset' && key !== 'color') ||
        typeof stop.offset !== 'number' || !Number.isFinite(stop.offset) || stop.offset < low || stop.offset > high || stop.offset < previous ||
        typeof stop.color !== 'string' || !(hex.test(stop.color) || isRole(stop.color))) invalid();
    previous = stop.offset;
    return { offset: stop.offset, color: stop.color };
  });
  const result = kind === 'linear' ? {} : { kind };
  for (const key of keys) if (value[key] !== undefined) result[key] = Array.isArray(value[key]) ? [...value[key]] : value[key];
  return { ...result, colorSpace: value.colorSpace ?? 'srgb', stops };
}
