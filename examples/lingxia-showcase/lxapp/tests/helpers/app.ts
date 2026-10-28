/**
 * The lxapp the page and flow specs drive. The GPU showcase runs these specs
 * too and names itself before importing them (its `tests/shared-setup.ts`).
 */
export const SHOWCASE_APP_ID: string =
  (globalThis as { __SHOWCASE_APP_ID__?: string }).__SHOWCASE_APP_ID__ ?? 'lingxia-showcase';
