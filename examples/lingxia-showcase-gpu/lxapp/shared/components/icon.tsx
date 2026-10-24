import type { LeafProps } from '@lingxia/ui';

// React showcase paths plus the monochrome ToDo note.
const paths = {
  note: ['M14 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-9', 'M16 3l3-2 4 4-2 3-9 9-5 1 1-5Z', 'M16 3l5 5', 'M7 7h5', 'M7 11h2'],
  alert: ['M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18Z', 'M12 7v6', 'M12 17h.01'],
  success: ['M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z'],
  bolt: ['M13 2L3 14h9l-1 8 10-12h-9l1-8z'],
  cloud: ['M6 19a4 4 0 0 1-.4-7.98A5.5 5.5 0 0 1 16 8.5h.5a4.5 4.5 0 1 1 .5 9H6z'],
  chevron: ['M9 18l6-6-6-6'],
  bridge: ['M8 7h12M8 12h12M8 17h12M4 7h.01M4 12h.01M4 17h.01'],
  navigation: ['M13 7l5 5-5 5M6 7l5 5-5 5'],
  interface: ['M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z', 'M9 22V12H15V22'],
  system: ['M4 4h16v16H4z'],
  exit: ['M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4', 'M10 17l5-5-5-5', 'M15 12H3'],
  device: ['M7 2H17A2 2 0 0 1 19 4V20A2 2 0 0 1 17 22H7A2 2 0 0 1 5 20V4A2 2 0 0 1 7 2Z', 'M12 18h.01'],
  media: ['M5 3L19 12L5 21Z'],
  file: ['M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z', 'M14 2V8H20'],
  location: ['M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z', 'M15 10A3 3 0 1 0 9 10A3 3 0 1 0 15 10Z'],
} as const;

export type IconName = keyof typeof paths;
export function Icon({ name, className = 'w-6 h-6 text-gray-400', ...props }: LeafProps & { name: IconName }) {
  return <vector {...props} className={`shrink-0 ${className}`} paths={paths[name].map(d => ({ d, fill: name === 'success' ? 'currentColor' : 'none', stroke: name === 'success' ? 'none' : 'currentColor', strokeWidth: 1.5 }))} />;
}
