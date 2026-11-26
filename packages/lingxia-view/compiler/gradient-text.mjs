// Text on a gradient with an app colour among its stops names an `on-` pair, measured against
// every opaque stop in each look the theme resolves to.

const channel = value => {
  const c = value / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
const luminance = hex => {
  const [r, g, b] = [1, 3, 5].map(at => channel(parseInt(hex.slice(at, at + 2), 16)));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const contrast = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

const base = rule => rule && !rule.error && !Object.keys(rule.when ?? {}).length;
const opaqueName = color => typeof color === 'string' && !color.startsWith('#') && !color.includes('/');

/**
 * Problems with the text colour of one element's classes over its gradient. `looks` maps each look
 * (`light`, `dark`, `high-contrast-light`, `high-contrast-dark`) to wire colour names and their hex;
 * without it only the pairing is checked.
 */
export function gradientTextProblems(names, rulesByName, looks) {
  const stops = [], texts = [];
  for (const name of names) for (const rule of rulesByName.get(name) ?? []) {
    if (!base(rule)) continue;
    for (const key of ['fromColor', 'viaColor', 'toColor']) if (rule.gradient?.[key]) stops.push({ name, color: rule.gradient[key] });
    if (rule.style?.color) texts.push({ name, color: rule.style.color });
  }
  const label = texts.at(-1);
  // Clipped text (`text-transparent`) is painted by the gradient itself.
  if (!label || /^#[0-9a-f]{6}00$/.test(label.color) || !stops.some(stop => stop.color.startsWith('app.'))) return [];
  const pair = label.color.replace(/^app\./, '');
  if (!pair.startsWith('on-') || !opaqueName(label.color)) {
    const stop = stops.find(stop => stop.color.startsWith('app.')).color.slice(4);
    return [`UI_COLOR: ${label.name} on a gradient over an app colour cannot be checked; name the stop's pair, such as text-on-${stop}`];
  }
  if (!looks) return [];
  let worst;
  for (const [look, colors] of Object.entries(looks)) {
    const needed = look.startsWith('high-contrast') ? 7 : 4.5;
    const ink = colors[label.color];
    for (const stop of stops) {
      const ground = opaqueName(stop.color) && colors[stop.color];
      if (!ink || !ground) continue;
      const ratio = contrast(ink, ground);
      if (ratio + 1e-9 < needed && (!worst || ratio < worst.ratio)) worst = { ratio, needed, look, stop: stop.name };
    }
  }
  return worst ? [`UI_COLOR: ${label.name} is ${worst.ratio.toFixed(2)}:1 on ${worst.stop} in ${worst.look}; text on a gradient needs ${worst.needed.toFixed(1)}:1 against every stop`] : [];
}
