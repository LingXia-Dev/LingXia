// Faces the page's theme declared with `@font-face`; the host loads and registers them.
let declared = [];

export function configureFonts(faces) {
  if (!Array.isArray(faces) || faces.length > 32) throw Error('UI_FONT: invalid font list');
  declared = faces.map(({ family, src, weight, style }) => ({ family: String(family), src: String(src), ...(weight ? { weight: String(weight) } : {}), ...(style ? { style: String(style) } : {}) }));
}

export const declaredFamily = name => declared.some(face => face.family === name);
export const takeFonts = () => JSON.stringify(declared);
