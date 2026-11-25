/**
 * The lxapp the page specs drive. The GPU showcase runs these specs
 * too and names itself before importing them (its `tests/shared-setup.ts`).
 */
export const SHOWCASE_APP_ID: string =
  (globalThis as { __SHOWCASE_APP_ID__?: string }).__SHOWCASE_APP_ID__ ?? 'lingxia-showcase';

/**
 * The run drives GPU pages, which draw Logic's overlays in the View. Keyed on the renderer the
 * showcase declares before importing these specs, not on its id, which the showcases share.
 */
export const GPU_MIRROR =
  (globalThis as { __SHOWCASE_RENDERER__?: string }).__SHOWCASE_RENDERER__ === 'gpu';
