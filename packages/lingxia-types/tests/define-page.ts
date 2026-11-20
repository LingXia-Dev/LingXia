import { definePage, type, type ParamsOf, type ResultOf } from '../src/page.js';
import type { PageContract } from '../src/page.js';

type Order = { id: string; total: number };

const order = definePage({
  params: type<{ id: string }>(),
  result: type<{ status: 'open' | 'done' }>(),
  data: { order: null as Order | null, n: 0 },
  setup(page) {
    let timer: number | undefined;
    return {
      onLoad({ id }) {
        const known: string = id;
        page.update(d => { d.order = { id: known, total: 0 }; });
      },
      onUnload() { if (timer !== undefined) clearInterval(timer); },
      actions: {
        inc() { page.update(d => { d.n += 1; }); },
        async markDone() { await page.close({ status: 'done' }); },
        rename(name: string) { return name.length; },
      },
    };
  },
});

// The definition is a page contract: View and tests read data and actions.
const asContract: PageContract<{ order: Order | null; n: number }, unknown> = order;
// Actions are Promises to the View.
const counted: (name: string) => Promise<number> = order.actions.rename;
type OrderParams = ParamsOf<typeof order>;
const params: OrderParams = { id: 'a' };
const done: ResultOf<typeof order> = { status: 'done' };

definePage({
  data: { n: 0 },
  setup: page => ({
    actions: {
      bad() {
        // @ts-expect-error a recipe is synchronous
        page.update(async d => { d.n = 1; });
        // @ts-expect-error a recipe changes the draft in place
        page.update(d => ({ ...d, n: 1 }));
        // @ts-expect-error data is readonly
        page.data.n = 2;
      },
    },
  }),
});

definePage({
  result: type<{ status: 'done' }>(),
  setup: page => ({
    actions: {
      // @ts-expect-error close takes the declared result
      wrong() { return page.close({ status: 'nope' }); },
    },
  }),
});

definePage({
  // @ts-expect-error put View-callable functions under actions
  setup: () => ({ save() {} }),
});

definePage({
  // @ts-expect-error a key a case away from a hook
  setup: () => ({ onUnLoad() {} }),
});

definePage({
  // @ts-expect-error setup is synchronous
  setup: async () => ({}),
});

definePage({
  // @ts-expect-error data is JSON
  data: { at: new Date() },
});

export type DefinePageGate = [typeof asContract, typeof counted, typeof params, typeof done];

definePage({
  setup(page) {
    const compact: boolean = page.env.sizeClass === 'compact';
    const stop: () => void = page.env.watch(({ sizeClass, canDock }) => {
      void sizeClass;
      void canDock;
    });
    // @ts-expect-error pixels stay in the View's env
    page.env.width;
    return { onUnload: () => stop(), onLoad: () => void compact };
  },
});
