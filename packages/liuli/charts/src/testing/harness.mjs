// The render helpers of Liuli core's test harness, bound to this package.
import { fileURLToPath } from 'node:url';
import { harnessFor } from '@liulidesign/core/internal/testing';

export const { renderPage, renderExample, described } = harnessFor(fileURLToPath(new URL('../..', import.meta.url)));

/**
 * Lays out each plot, as a host does after the first frame: a chart draws its lines and bars in
 * once its plot has a size, and stands empty before that.
 */
export function laidOut(screen, box = { width: 320, height: 160 }) {
  const holdsDrawing = node => node.children.some(child => child.tag === 'vector' || holdsDrawing(child));
  for (const node of screen.queryAll(each => each.tag === 'column' && each.props.onLayout !== undefined && holdsDrawing(each))) screen.layout(node, box);
  return screen;
}
