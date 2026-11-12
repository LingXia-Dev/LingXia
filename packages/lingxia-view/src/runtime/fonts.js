// Faces the page's theme declared with `@font-face`; the host loads and registers them.
let declared = [];
/** The theme's `--font-sans` when it is not the default: the page root's family. */
export let pageFamily;

export function configureFonts(faces, family) {
  if (!Array.isArray(faces) || faces.length > 32) throw Error('UI_FONT: invalid font list');
  if (family !== undefined && typeof family !== 'string') throw Error('UI_FONT: invalid page family');
  declared = faces.map(({ family, src, weight, style }) => ({ family: String(family), src: String(src), ...(weight ? { weight: String(weight) } : {}), ...(style ? { style: String(style) } : {}) }));
  pageFamily = family;
}

export const declaredFamily = name => declared.some(face => face.family === name);
export const takeFonts = () => JSON.stringify(declared);
