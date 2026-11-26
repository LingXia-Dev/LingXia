import type { IconDefinition } from '@liulidesign/core/Icon';

/** Icons the set has no name for. */
export const editorIcons = {
  // A solid opening quote: outlined or boxed marks read as the digits 99 at toolbar size.
  quote: {
    paths: [{ d: 'M4 15.2C4 10.6 6.4 7.9 10 7.2L10.3 8.7C8.2 9.3 6.65 11.21 6.45 12.01A3.3 3.3 0 1 1 4 15.2zM13.4 15.2C13.4 10.6 15.8 7.9 19.4 7.2L19.7 8.7C17.6 9.3 16.05 11.21 15.85 12.01A3.3 3.3 0 1 1 13.4 15.2z', fill: 'currentColor' }],
  },
} as const satisfies Readonly<Record<string, IconDefinition>>;
