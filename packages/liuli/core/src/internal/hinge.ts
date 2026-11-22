import type { HingeRect } from '@lingxia/view';

interface HingeEnv {
  readonly hinge?: HingeRect | null;
  readonly viewport?: { readonly width: number; readonly height: number };
  readonly insets?: { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number };
}

/**
 * The page's hinge in its content box, where a full-page layout starts: `env.hinge` is
 * canvas-local, but the page pads its content by `env.insets`, so panes sized from the raw
 * rect would miss the crease by the inset (by the whole top bar in tabletop posture).
 */
export function contentHinge(env: HingeEnv): { hinge: HingeRect | null; width: number; height: number } {
  const insets = env.insets ?? { top: 0, right: 0, bottom: 0, left: 0 };
  const width = Math.max(0, (env.viewport?.width ?? 0) - insets.left - insets.right);
  const height = Math.max(0, (env.viewport?.height ?? 0) - insets.top - insets.bottom);
  const raw = env.hinge ?? null;
  const hinge = raw && { ...raw, x: raw.x - insets.left, y: raw.y - insets.top };
  return { hinge, width, height };
}
