/** Snap positions the page decides: a pager and a panel whose index lives in page data. */
export interface HeldSnap { page: number; panel: number }
export interface HeldChoice { index: number; slow: boolean }

export const heldSnap = (): HeldSnap => ({ page: 0, panel: 1 });

// Long enough to see the wait on a fast phone; well under the element's 400 ms wait.
const SLOW_MS = 250;

type Holder = { update(change: (data: { held: HeldSnap }) => void): void };

const answer = async (slow: boolean) => {
  if (slow) await new Promise<void>(resolve => setTimeout(resolve, SLOW_MS));
};

/** Takes the pager's settled page, except the last one, which it refuses. */
export async function holdPage(page: Holder, { index, slow }: HeldChoice) {
  await answer(slow);
  if (index === 2) throw new Error('The last page is closed');
  page.update(d => { d.held.page = index; });
}

/** Takes the panel's settled point, except the top, which it refuses. */
export async function holdPanel(page: Holder, { index, slow }: HeldChoice) {
  await answer(slow);
  if (index === 0) throw new Error('The panel does not open all the way');
  page.update(d => { d.held.panel = index; });
}
