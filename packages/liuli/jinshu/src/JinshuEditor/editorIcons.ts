import type { IconDefinition } from '@liulidesign/core/Icon';

/** Crisp 24-grid stroke icons matching Liuli's Lucide stroke geometry (1.75px round caps/joins). */
export const editorIcons = {
  heading: {
    paths: ['M6 5v14', 'M18 5v14', 'M6 12h12'],
  },
  bold: {
    paths: ['M7 5h6.5a3.5 3.5 0 0 1 0 7H7V5z', 'M7 12h7.5a3.5 3.5 0 0 1 0 7H7v-7z'],
  },
  italic: {
    paths: ['M10 5h6', 'M8 19h6', 'M14 5l-4 14'],
  },
  underline: {
    paths: ['M7 5v6a5 5 0 0 0 10 0V5', 'M5 19h14'],
  },
  strike: {
    paths: ['M16 7.5A4 4 0 0 0 12 5c-2.5 0-4 1.3-4 3 0 4 8 2.5 8 7 0 2-1.8 4-4.5 4a4.5 4.5 0 0 1-4.2-2.5', 'M4 12h16'],
  },
  code: {
    paths: ['m8 8-4 4 4 4', 'm16 8 4 4-4 4'],
  },
  codeBlock: {
    paths: ['M8 4H6a2 2 0 0 0-2 2v4a2 2 0 0 1-2 2 2 2 0 0 1 2 2v4a2 2 0 0 0 2 2h2', 'M16 4h2a2 2 0 0 1 2 2v4a2 2 0 0 0 2 2 2 2 0 0 0-2 2v4a2 2 0 0 1-2 2h-2'],
  },
  quote: {
    paths: ['M10 8H6v5h4V8zm0 5c0 2.5-1.5 4-3.5 4.5', 'M18 8h-4v5h4V8zm0 5c0 2.5-1.5 4-3.5 4.5'],
  },
  highlight: {
    paths: ['m9 11 6-6 4 4-6 6H9v-4z', 'M5 19h14', 'm9 15-3 3'],
  },
  superscript: {
    paths: ['m4 9 7 8', 'm11 9-7 8', 'M16 6.5a2 2 0 0 1 3.8.8c0 1.4-3.8 2.7-3.8 4.2h4'],
  },
  subscript: {
    paths: ['m4 7 7 8', 'm11 7-7 8', 'M16 14.5a2 2 0 0 1 3.8.8c0 1.4-3.8 2.7-3.8 4.2h4'],
  },
  math: {
    paths: ['M18 6V5H6l6 7-6 7h12v-1'],
  },
  table: {
    paths: ['M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z', 'M3 10h18', 'M9 5v14', 'M15 5v14'],
  },
  alignStart: {
    paths: ['M4 6h16', 'M4 10h10', 'M4 14h16', 'M4 18h12'],
    isMirrored: true,
  },
  alignCenter: {
    paths: ['M4 6h16', 'M7 10h10', 'M4 14h16', 'M6 18h12'],
  },
  alignEnd: {
    paths: ['M4 6h16', 'M10 10h10', 'M4 14h16', 'M8 18h12'],
    isMirrored: true,
  },
  divider: {
    paths: ['M3 12h18'],
  },
} as const satisfies Readonly<Record<string, IconDefinition>>;
