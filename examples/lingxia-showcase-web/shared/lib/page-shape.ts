/** The data and actions a page's Logic and its React View both type against. */
export interface PageShape<Data, Actions> {
  readonly data: Data;
  readonly actions: Actions;
}
