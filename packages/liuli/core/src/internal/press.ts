import type { LinkTarget } from '@lingxia/view';

/**
 * A control's press: its own actions, or a place to go, never both. To decide first or wait for a
 * result before going, use an action.
 */
export type Press<Actions> =
  | (Actions & { to?: never })
  | ({
      /** Makes it a link that goes there on press: `{ page, params }`, `{ tab }`, `{ back: 1 }`. Read as a link; Enter opens it and Space does not. */
      to: LinkTarget;
    } & { [K in keyof Actions]?: never });
