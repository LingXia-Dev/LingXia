/** A shared, type-only page contract. Importing it installs no Logic globals. */
export interface PageContract<Data = unknown, Actions = unknown> {
  readonly data: Data;
  readonly actions: Actions;
}
