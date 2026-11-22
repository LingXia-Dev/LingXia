/** The data and actions the home page's Logic and View share. */
export interface HomePage {
  readonly data: { greeting: string; greetCount: number };
  readonly actions: { greet(payload: { name: string }): Promise<void> };
}
